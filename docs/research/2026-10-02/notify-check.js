// notify-check：桌面通知與通知設定面板驗收（desktop-launch-notify task 3.2；design D7、D8、D9、D10）。
// 各段代號、DOM 約定與對應的 spec scenario 見同目錄 notify-check.md。
//
// 權威：openspec/changes/desktop-launch-notify/specs/desktop-notifications/spec.md（「通知事件」「通知呈現」
// 「通知設定」全部 scenario）與 specs/cockpit-dashboard/spec.md（`/app/notify.js` 路由、「鈴鐺按鈕跨重畫保留焦點」
// 「鈴鐺不影響畫面操作的錯誤訊息」）。
//
// 做法：
//   - 以 CDP `Page.addScriptToEvaluateOnNewDocument` 在每次載入前換掉 `window.Notification`（記錄器：
//     記下 title／body／tag／renotify，權限三種值與「不支援」可切換，`requestPermission` 記次數），
//     覆寫 `document.hasFocus`、`document.visibilityState`、`window.focus`（記次數），並包一層 WebSocket
//     記下每條連線的第一則訊息（重連段的前提檢查用）。設定存在 localStorage 鍵 `__cockpitNotifyTest`，
//     重新載入後保留。
//   - 狀態轉換由 ui_preview 的 `COCKPIT_PREVIEW_TRANSITIONS` 排程（design D9），每段各起一個 ui_preview；
//     轉換時間從 ui_preview 啟動起算，腳本以 `/api/state` 輪詢確認轉換已套用，再讀記錄器。
//   - 鈴鐺與面板一律用 `Input.dispatchMouseEvent`／`Input.dispatchKeyEvent` 的真實輸入（不用 el.click()）。
//   - 通知的「點選」只能在頁面內呼叫記錄器實例的 click（作業系統的通知無法自動化）。
//
// 分段：
//   N 段（通知事件／呈現，ui_preview 一）：第一份狀態不通知、agent 卡住（標題、內文、tag、renotify）、
//      預設關閉的類別、同 pane 同類取代、點通知帶出 pane、pane 已消失與改綁模式中點通知不改選取。
//   M 段（ui_preview 二＋重啟）：前景不打擾、頁面不可見時照發、合併通知、面板開啟 completed／done 後發出、
//      task 通知內文、點 task 通知、重連後與斷線前最後一份比對（真的停掉再重啟 ui_preview）。
//   S 段（ui_preview 三）：`/app/notify.js` 路由、鈴鐺鍵盤焦點跨重畫、面板開啟焦點、預設值、標籤與禁字、
//      對比與焦點外框、減少動態、重畫不關閉面板、Esc／鈴鐺／點外面關閉與焦點回鈴鐺、設定保留、損毀與不可用、
//      權限三態與不支援、鈴鐺不清錯誤訊息與不離開改綁模式。
//   P 段（S 段的頁面內）：直接呼叫 `cockpitNotify.diff`／`toNotifications` 驗邊界。
//   T 段（ui_preview 四）：同一個瀏覽器、同源的兩個分頁（啟動器會開出多個同源視窗）：甲改設定 → 乙開著的面板與
//      乙之後發通知用的設定跟著變；乙再改另一個開關 → 不蓋掉甲先前的變更（spec「通知設定」變更立即生效；
//      5.3 審查 I2）。
//
// 輸出：每條斷言一行 `PASS`／`FAIL`；「前置：」開頭的是環境與既有行為的前提（不是本 change 的新行為），
// 結尾另列新行為斷言的通過／失敗數，最後一行 `RESULT: PASS` 或 `RESULT: FAIL (N)`；有失敗時結束碼非 0。
//
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-10-02/notify-check.js [--only=N,M,S,T] [--screenshots]
// --screenshots（desktop-launch-notify task 3.4）：全部斷言跑完後，以真實滑鼠點鈴鐺開啟設定面板，產生 1536／700 寬的
// notify-panel-<寬>.png（權限「尚未決定」、四個開關為預設值）；截圖前遮罩真實使用者與主機名稱（見 screenshotCase）。
// 只要截圖可加 --only=none（不跑任何斷言段）。
// 清理：只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID），埠被占用就往上找空埠。
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

// ---------------------------------------------------------------------------
// 斷言與記錄
// ---------------------------------------------------------------------------

const failures = [];
const tally = { pre: { pass: 0, fail: 0 }, spec: { pass: 0, fail: 0 } };
// spec 斷言（本 change 的新行為）。
function check(cond, label) {
  const ok = !!cond;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${label}`);
  tally.spec[ok ? 'pass' : 'fail'] += 1;
  if (!ok) failures.push(label);
  return ok;
}
// 前置（環境、fixture、既有行為）：失敗同樣算 FAIL，但分開計數。
function pre(cond, label) {
  const ok = !!cond;
  console.log(`${ok ? 'PASS' : 'FAIL'} 前置：${label}`);
  tally.pre[ok ? 'pass' : 'fail'] += 1;
  if (!ok) failures.push(`前置：${label}`);
  return ok;
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const J = (v) => JSON.stringify(v);

// ---------------------------------------------------------------------------
// 行程與埠（同 ui-fixes-check.js）
// ---------------------------------------------------------------------------

function pidStillRunning(pid) {
  const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
  return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
}

function killTree(child, label) {
  if (!child || child.exitCode !== null) return;
  spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
  pre(!pidStillRunning(child.pid), `${label} PID ${child.pid} 已終止（tasklist 查無此 PID）`);
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

// ---------------------------------------------------------------------------
// CDP
// ---------------------------------------------------------------------------

class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.exceptions = []; // Runtime.exceptionThrown（未捕捉例外與未處理的 rejection）
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        this.pending.get(m.id)(m);
        this.pending.delete(m.id);
      } else if (m.method === 'Runtime.exceptionThrown') {
        const d = m.params.exceptionDetails || {};
        this.exceptions.push({
          at: Date.now(),
          text: `${d.text || ''} ${(d.exception && d.exception.description) || ''}`.trim().slice(0, 300),
          url: d.url || '',
        });
      }
    };
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((res) => {
      this.pending.set(id, res);
      this.ws.send(J({ id, method, params }));
    });
  }
  async eval(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.result && r.result.exceptionDetails) {
      throw new Error(`頁面內例外：${J(r.result.exceptionDetails).slice(0, 400)}`);
    }
    return r.result && r.result.result ? r.result.result.value : undefined;
  }
  // 在頁面內呼叫一個（可為 async 的）函式；函式本體以 toString 序列化，參數以 JSON 傳入。
  fn(f, ...args) {
    return this.eval(`(${f.toString()})(...${J(args)})`);
  }
  async waitFor(expression, timeoutMs) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      if (await this.eval(expression)) return true;
      await sleep(50);
    }
    return false;
  }
  async mouseAt(x, y) {
    const base = { x, y, button: 'left', clickCount: 1 };
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
  }
  // 以真的滑鼠事件點「頁面內運算式取得的元素」的中心；找不到回 false。
  async clickExpr(elementExpr) {
    const rect = await this.eval(
      `(() => { const n = (${elementExpr}); if (!n) return null; n.scrollIntoView({block: 'nearest'});
        const r = n.getBoundingClientRect(); if (r.width === 0 || r.height === 0) return null;
        return {x: r.left + r.width / 2, y: r.top + r.height / 2}; })()`
    );
    if (!rect) return false;
    await this.mouseAt(rect.x, rect.y);
    return true;
  }
  click(selector) {
    return this.clickExpr(`document.querySelector(${J(selector)})`);
  }
  // 真實按鍵（keyDown＋keyUp）。
  async press(name) {
    const keys = {
      Tab: { key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 },
      Enter: { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' },
      Space: { key: ' ', code: 'Space', windowsVirtualKeyCode: 32, text: ' ' },
      Escape: { key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 },
      // Shift+Tab（修正波 3.6 M9）：modifiers 8＝Shift。
      ShiftTab: { key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9, modifiers: 8 },
    };
    const k = keys[name];
    await this.send('Input.dispatchKeyEvent', { type: 'keyDown', ...k });
    const { text, ...up } = k;
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', ...up });
  }
}

// ---------------------------------------------------------------------------
// 頁面載入前注入：Notification 記錄器、焦點與可見性、WebSocket 第一則訊息
// ---------------------------------------------------------------------------

// 這個函式以 toString 序列化後注入頁面，不能引用外部變數。
function fakeInit() {
  // about:blank 等非 http 頁面不注入（沒有本機儲存可用）。
  if (location.protocol !== 'http:') return;
  const KEY = '__cockpitNotifyTest';
  const ls = window.localStorage;
  const origGet = Storage.prototype.getItem;
  const origSet = Storage.prototype.setItem;
  let saved = {};
  try {
    saved = JSON.parse(origGet.call(ls, KEY) || '{}') || {};
  } catch (e) {
    saved = {};
  }
  const cfg = Object.assign(
    { permission: 'granted', requestResult: 'granted', focused: false, visibility: null, unsupported: false, storageThrows: false },
    saved
  );
  window.__notifyCfg = cfg;
  window.__setNotifyCfg = (patch) => {
    Object.assign(cfg, patch);
    origSet.call(ls, KEY, JSON.stringify(cfg));
    return true;
  };
  // 測試腳本自己清空與寫入本機儲存（storageThrows 時頁面上的 localStorage 不可用）。
  window.__lsClear = () => {
    ls.clear();
    origSet.call(ls, KEY, JSON.stringify(cfg));
    return true;
  };
  window.__lsSet = (k, v) => {
    origSet.call(ls, k, v);
    return true;
  };
  window.__lsGet = (k) => origGet.call(ls, k);

  window.__notifs = [];
  window.__notifInstances = [];
  window.__permRequests = 0;
  window.__focusCalls = 0;
  window.focus = function () {
    window.__focusCalls += 1;
  };
  document.hasFocus = function () {
    return !!cfg.focused;
  };
  const visDesc = Object.getOwnPropertyDescriptor(Document.prototype, 'visibilityState');
  Object.defineProperty(document, 'visibilityState', {
    configurable: true,
    get() {
      return cfg.visibility || visDesc.get.call(document);
    },
  });
  Object.defineProperty(document, 'hidden', {
    configurable: true,
    get() {
      return document.visibilityState === 'hidden';
    },
  });

  if (cfg.storageThrows) {
    // 本機儲存不可用：存取 window.localStorage 本身就丟 SecurityError（同瀏覽器封鎖網站資料時）。
    Object.defineProperty(window, 'localStorage', {
      configurable: true,
      get() {
        throw new DOMException('模擬：本機儲存不可用', 'SecurityError');
      },
    });
  }

  if (cfg.unsupported) {
    delete window.Notification;
  } else {
    class FakeNotification extends EventTarget {
      constructor(title, options) {
        super();
        const o = options || {};
        this.title = String(title);
        this.body = o.body;
        this.tag = o.tag;
        this.renotify = o.renotify;
        this.onclick = null;
        this.onclose = null;
        this.__rec = {
          idx: window.__notifs.length,
          title: String(title),
          body: o.body === undefined ? null : o.body,
          tag: o.tag === undefined ? null : o.tag,
          renotify: o.renotify === undefined ? null : o.renotify,
          silent: o.silent === undefined ? null : o.silent,
          permission: cfg.permission,
          focused: !!cfg.focused,
          visibility: document.visibilityState,
          closed: false,
          at: Date.now(),
        };
        window.__notifs.push(this.__rec);
        window.__notifInstances.push(this);
      }
      static get permission() {
        return cfg.permission;
      }
      static requestPermission(cb) {
        window.__permRequests += 1;
        return new Promise((resolve) => {
          setTimeout(() => {
            window.__setNotifyCfg({ permission: cfg.requestResult });
            if (typeof cb === 'function') cb(cfg.permission);
            resolve(cfg.permission);
          }, 50);
        });
      }
      close() {
        this.__rec.closed = true;
      }
      // 模擬使用者點選這則通知。
      __click() {
        const e = new Event('click', { cancelable: true });
        this.dispatchEvent(e);
        if (typeof this.onclick === 'function') this.onclick.call(this, e);
      }
    }
    Object.defineProperty(window, 'Notification', { configurable: true, writable: true, value: FakeNotification });
  }

  // 每條 /ws 連線的第一則訊息（只留 cockpit/ops-2 的 status，供 M 段重連的前提檢查）。
  const OrigWS = window.WebSocket;
  let seq = 0;
  window.__wsFirst = [];
  function WrappedWS(url, protocols) {
    const ws = protocols === undefined ? new OrigWS(url) : new OrigWS(url, protocols);
    const id = ++seq;
    let first = true;
    ws.addEventListener('message', (e) => {
      if (!first) return;
      first = false;
      let ops2Status = null;
      try {
        const s = JSON.parse(e.data);
        const p = (s.projects || []).find((x) => x.id === 'cockpit');
        const t = p && (p.tasks || []).find((x) => x.id === 'ops-2');
        ops2Status = t ? t.status : null;
      } catch (err) {
        ops2Status = 'unparsable';
      }
      window.__wsFirst.push({ id, ops2Status, at: Date.now() });
    });
    return ws;
  }
  WrappedWS.prototype = OrigWS.prototype;
  for (const k of ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED']) WrappedWS[k] = OrigWS[k];
  window.WebSocket = WrappedWS;
}

// ---------------------------------------------------------------------------
// Chrome 與 ui_preview
// ---------------------------------------------------------------------------

async function startChrome(cdpPort) {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-chrome-notify-'));
  const chrome = spawn(
    CHROME,
    [
      '--headless=new',
      '--disable-gpu',
      '--no-first-run',
      `--remote-debugging-port=${cdpPort}`,
      '--remote-allow-origins=*',
      `--user-data-dir=${udd}`,
      '--window-size=1400,1000',
      'about:blank',
    ],
    { stdio: 'ignore', windowsHide: true }
  );
  chrome.on('error', (e) => pre(false, `chrome spawn error：${e.message}`));
  let page = null;
  for (let i = 0; i < 100 && !page; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      page = (await r.json()).find((t) => t.type === 'page');
    } catch {
      // CDP endpoint 還沒起來。
    }
    if (!page) await sleep(200);
  }
  // 等不到 page target 或 WebSocket 時，先收掉自己開的 chrome 與暫存目錄再中止。
  const abort = async (msg) => {
    killTree(chrome, 'chrome（啟動失敗）');
    await sleep(500);
    fs.rmSync(udd, { recursive: true, force: true });
    throw new Error(msg);
  };
  if (!page) await abort('chrome: page target not found');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  try {
    await new Promise((res, rej) => {
      ws.onopen = res;
      ws.onerror = rej;
    });
  } catch {
    await abort('chrome: CDP WebSocket 連不上');
  }
  const cdp = new CDP(ws);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: `(${fakeInit.toString()})();` });
  return { chrome, udd, ws, cdp, cdpPort };
}

async function stopChrome(handle) {
  if (!handle) return;
  try {
    handle.ws.close();
  } catch {
    // 已斷線。
  }
  killTree(handle.chrome, 'chrome');
  await sleep(500);
  try {
    fs.rmSync(handle.udd, { recursive: true, force: true });
  } catch (e) {
    pre(false, `清理暫存目錄失敗：${e.message}`);
  }
}

// 起一個 ui_preview；transitions 為規則陣列（以 ; 串接）。回傳 { proc, spawnAt }。
function startServer(port, { transitions = [], writeRules = [] } = {}) {
  const env = {
    ...process.env,
    COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`,
    COCKPIT_PREVIEW_PUSH_MS: '100',
  };
  if (transitions.length) env.COCKPIT_PREVIEW_TRANSITIONS = transitions.join(';');
  if (writeRules.length) env.COCKPIT_PREVIEW_WRITE_RULES = writeRules.join(';');
  // 只在 N 段用：pane 消失。
  if (startServer.vanish) env.COCKPIT_PREVIEW_VANISH_PANE = startServer.vanish;
  const proc = spawn(UI_PREVIEW_EXE, [], { stdio: ['ignore', 'ignore', 'pipe'], windowsHide: true, env });
  let stderr = '';
  proc.stderr.setEncoding('utf8');
  proc.stderr.on('data', (c) => {
    stderr += c;
  });
  proc.on('error', (e) => pre(false, `ui_preview spawn error：${e.message}`));
  return { proc, spawnAt: Date.now(), stderr: () => stderr };
}

async function waitUp(port) {
  for (let i = 0; i < 50; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/api/state`)).ok) return true;
    } catch {
      // 還沒起來。
    }
    await sleep(200);
  }
  return false;
}

async function apiState(port) {
  const r = await fetch(`http://127.0.0.1:${port}/api/state`);
  return r.json();
}

function findPane(s, runtime, pane) {
  for (const r of s.runtimes || []) {
    if (r.id !== runtime) continue;
    for (const w of r.workspaces || []) for (const t of w.tabs || []) for (const p of t.panes || []) if (p.id === pane) return p;
  }
  return null;
}
function findTask(s, project, task) {
  const p = (s.projects || []).find((x) => x.id === project);
  return p ? (p.tasks || []).find((x) => x.id === task) || null : null;
}
const paneIs = (runtime, pane, status) => (s) => {
  const p = findPane(s, runtime, pane);
  return !!p && p.agent_status === status;
};
const taskIs = (project, task, status) => (s) => {
  const t = findTask(s, project, task);
  return !!t && t.status === status;
};

// 等 ui_preview 套用某個轉換（以 /api/state 判斷），再等頁面收到推送並處理。
async function waitServer(port, pred, label, settleMs = 700) {
  const start = Date.now();
  while (Date.now() - start < 30000) {
    try {
      if (pred(await apiState(port))) {
        await sleep(settleMs);
        return true;
      }
    } catch {
      // 暫時連不上。
    }
    await sleep(80);
  }
  pre(false, `逾時：等 ui_preview 套用轉換（${label}）`);
  return false;
}

// ---------------------------------------------------------------------------
// 頁面內讀取
// ---------------------------------------------------------------------------

const BELL = '#app [data-action="notify-settings"]';
const KINDS = ['blocked', 'done', 'failed', 'completed'];
const LABELS = { blocked: 'agent blocked', done: 'agent done', failed: 'task failed', completed: 'task completed' };
const DEFAULTS = { blocked: true, done: false, failed: true, completed: false };
const STORAGE_KEY = 'cockpit.notify.v1';

const versionOf = "Number(document.getElementById('version').textContent.replace(/\\D/g, ''))";
const moduleLoaded = "(typeof window.cockpitNotify === 'object' && window.cockpitNotify !== null && typeof window.cockpitNotify.observe === 'function')";

// 面板：body 底下、#app 之外的 #notify-panel；「開啟」＝存在、沒有 hidden、有算出的尺寸。
function panelInfo() {
  const p = document.getElementById('notify-panel');
  if (!p) {
    return { exists: false, open: false, insideApp: false, inBody: false, kinds: [], state: {}, text: '', attrText: '', buttons: [], focusInPanel: false, focusKind: null, focusIsFirstToggle: false };
  }
  const cs = getComputedStyle(p);
  const r = p.getBoundingClientRect();
  const open = !p.hidden && cs.display !== 'none' && cs.visibility !== 'hidden' && r.width > 0 && r.height > 0;
  const toggles = Array.from(p.querySelectorAll('[data-notify-kind]'));
  const state = {};
  for (const t of toggles) {
    state[t.dataset.notifyKind] = t.type === 'checkbox' ? t.checked : t.getAttribute('aria-checked') === 'true';
  }
  const a = document.activeElement;
  const buttons = Array.from(p.querySelectorAll('button')).map((b) => b.textContent.trim());
  return {
    exists: true,
    open,
    insideApp: !!p.closest('#app'),
    inBody: document.body.contains(p),
    kinds: toggles.map((t) => t.dataset.notifyKind),
    state,
    text: p.textContent.replace(/\s+/g, ' ').trim(),
    attrText: Array.from(p.querySelectorAll('[title], [aria-label]'))
      .map((n) => `${n.getAttribute('title') || ''} ${n.getAttribute('aria-label') || ''}`)
      .join(' '),
    buttons,
    focusInPanel: !!a && p.contains(a),
    focusKind: a && a.dataset ? a.dataset.notifyKind || null : null,
    focusIsFirstToggle: toggles.length > 0 && a === toggles[0],
  };
}
const panelExpr = `(${panelInfo.toString()})()`;
const panelOpenExpr = `(${panelInfo.toString()})().open`;

function focusInfo(bellSel) {
  const a = document.activeElement;
  if (!a || a === document.body) return { body: true, onBell: false, fv: false };
  return { body: false, onBell: a.matches(bellSel), fv: a.matches(':focus-visible'), tag: a.tagName, action: a.dataset ? a.dataset.action || null : null };
}
const focusExpr = `(${focusInfo.toString()})(${J(BELL)})`;

const notifsExpr = 'window.__notifs ? window.__notifs.slice() : []';
const notifCount = 'window.__notifs ? window.__notifs.length : -1';

// 點選某則通知：tag 相同的最後一則記錄器實例。
function clickNotification(tag) {
  const list = window.__notifInstances || [];
  for (let i = list.length - 1; i >= 0; i--) {
    if (list[i].__rec.tag === tag) {
      const before = window.__focusCalls;
      list[i].__click();
      return { found: true, idx: list[i].__rec.idx, focusDelta: window.__focusCalls - before, closed: list[i].__rec.closed };
    }
  }
  return { found: false };
}

// 右欄選定標示與 Live Output 標題。
function selectionInfo() {
  const rows = Array.from(document.querySelectorAll('#app .pane-row.selected')).map((r) => `${r.dataset.runtime}/${r.dataset.pane}`);
  const t = document.querySelector('#output .output-title');
  return { rows, title: t ? t.textContent : null };
}
const selectionExpr = `(${selectionInfo.toString()})()`;

// 面板內文字對比（文字色疊到祖先鏈實際背景上，比 WCAG 對比 ≥ 4.5）。
function contrastProbe() {
  const p = document.getElementById('notify-panel');
  if (!p) return null;
  const cv = document.createElement('canvas');
  cv.width = 1;
  cv.height = 1;
  const cx = cv.getContext('2d', { willReadFrequently: true });
  const rgba = (c) => {
    cx.clearRect(0, 0, 1, 1);
    cx.fillStyle = '#000';
    cx.fillStyle = c;
    cx.fillRect(0, 0, 1, 1);
    const d = cx.getImageData(0, 0, 1, 1).data;
    return { r: d[0], g: d[1], b: d[2], a: d[3] / 255 };
  };
  const over = (c, base) => ({ r: c.r * c.a + base.r * (1 - c.a), g: c.g * c.a + base.g * (1 - c.a), b: c.b * c.a + base.b * (1 - c.a), a: 1 });
  const lum = (c) => {
    const f = (v) => {
      const s = v / 255;
      return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
  };
  const ratio = (a, b) => {
    const x = lum(a);
    const y = lum(b);
    return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
  };
  const bgOf = (el) => {
    const chain = [];
    for (let n = el; n; n = n.parentElement) chain.push(n);
    let base = { r: 255, g: 255, b: 255, a: 1 };
    for (let i = chain.length - 1; i >= 0; i--) {
      const c = rgba(getComputedStyle(chain[i]).backgroundColor);
      if (c.a > 0) base = over(c, base);
    }
    return base;
  };
  const opacityOf = (el) => {
    let o = 1;
    for (let n = el; n; n = n.parentElement) o *= parseFloat(getComputedStyle(n).opacity);
    return o;
  };
  const out = [];
  for (const el of [p, ...p.querySelectorAll('*')]) {
    const own = Array.from(el.childNodes).some((n) => n.nodeType === 3 && n.nodeValue.trim() !== '');
    if (!own) continue;
    const cs = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    if (cs.visibility === 'hidden' || cs.display === 'none' || r.width === 0 || r.height === 0) continue;
    const bg = bgOf(el);
    const fg = rgba(cs.color);
    fg.a *= opacityOf(el);
    out.push({ text: el.textContent.trim().slice(0, 30), ratio: Math.round(ratio(over(fg, bg), bg) * 100) / 100 });
  }
  return out;
}

// 焦點外框：目前焦點元素（或它的 label）有 ≥2px 的 outline 或 box-shadow，且元素有尺寸。
function focusRing() {
  const a = document.activeElement;
  if (!a || a === document.body) return null;
  const cands = [a, ...(a.labels ? Array.from(a.labels) : []), a.closest('label')].filter(Boolean);
  const res = cands.map((n) => {
    const cs = getComputedStyle(n);
    const r = n.getBoundingClientRect();
    return { tag: n.tagName, w: r.width, h: r.height, os: cs.outlineStyle, ow: parseFloat(cs.outlineWidth) || 0, oc: cs.outlineColor, bs: cs.boxShadow };
  });
  const ok = res.some(
    (x) => x.w > 0 && x.h > 0 && ((x.os !== 'none' && x.ow >= 2 && x.oc !== 'rgba(0, 0, 0, 0)') || (x.bs && x.bs !== 'none'))
  );
  return { fv: a.matches(':focus-visible'), ok, res };
}

// 減少動態：面板與其子孫沒有動畫、transition 時間為 0。
function motionProbe() {
  const p = document.getElementById('notify-panel');
  if (!p) return null;
  const bad = [];
  for (const el of [p, ...p.querySelectorAll('*')]) {
    const cs = getComputedStyle(el);
    const durs = cs.transitionDuration.split(',').map((s) => parseFloat(s) || 0);
    if (cs.animationName !== 'none' || durs.some((d) => d > 0)) bad.push(`${el.tagName}.${el.className}:${cs.animationName}/${cs.transitionDuration}`);
  }
  return bad;
}

// 開關的可及名稱（label、aria-label、aria-labelledby、外層 label）。
function toggleNames() {
  const p = document.getElementById('notify-panel');
  if (!p) return null;
  const out = {};
  for (const t of p.querySelectorAll('[data-notify-kind]')) {
    const parts = [];
    if (t.labels) for (const l of t.labels) parts.push(l.textContent);
    if (t.getAttribute('aria-label')) parts.push(t.getAttribute('aria-label'));
    const lb = t.getAttribute('aria-labelledby');
    if (lb) for (const id of lb.split(/\s+/)) parts.push((document.getElementById(id) || {}).textContent || '');
    const outer = t.closest('label');
    if (outer) parts.push(outer.textContent);
    if (t.tagName === 'BUTTON') parts.push(t.textContent);
    out[t.dataset.notifyKind] = parts.join(' ').replace(/\s+/g, ' ').trim();
  }
  return out;
}

// 鈴鐺的 aria-expanded、計算樣式與尺寸，以及 --accent 解析後的顏色與四個開關的 accent-color
// （修正波：3.5 採納項、3.6 M2）。
function bellProbe(sel) {
  const b = document.querySelector(sel);
  if (!b) return null;
  const probe = document.createElement('span');
  probe.style.color = 'var(--accent)';
  document.body.appendChild(probe);
  const accent = getComputedStyle(probe).color;
  probe.remove();
  const cs = getComputedStyle(b);
  const r = b.getBoundingClientRect();
  const bar = b.closest('[data-region="topbar"]');
  const t = bar ? bar.getBoundingClientRect() : { top: 0, height: 0 };
  const boxes = Array.from(document.querySelectorAll('#notify-panel [data-notify-kind]')).map((x) => getComputedStyle(x).accentColor);
  return {
    expanded: b.getAttribute('aria-expanded'),
    border: cs.borderTopColor,
    color: cs.color,
    accent,
    w: Math.round(r.width * 100) / 100,
    h: Math.round(r.height * 100) / 100,
    dy: Math.round((r.top + r.height / 2 - (t.top + t.height / 2)) * 100) / 100,
    boxes,
  };
}
const bellProbeExpr = `(${bellProbe.toString()})(${J(BELL)})`;

// 面板、第一個開關與鈴鐺相對視窗的位置（修正波 3.6 M4）。
function panelViewport(sel) {
  const p = document.getElementById('notify-panel');
  const b = document.querySelector(sel);
  const first = p ? p.querySelector('[data-notify-kind]') : null;
  const box = (n) => {
    if (!n) return null;
    const r = n.getBoundingClientRect();
    return { top: Math.round(r.top), bottom: Math.round(r.bottom) };
  };
  const vh = window.innerHeight;
  const panel = box(p);
  const toggle = box(first);
  const inView = (x) => !!x && x.top >= 0 && x.bottom <= vh;
  return { vh, scrollY: Math.round(window.scrollY), panel, toggle, bell: box(b), panelIn: inView(panel), toggleIn: inView(toggle) };
}
const panelViewportExpr = `(${panelViewport.toString()})(${J(BELL)})`;

// 以 /api/state 的兩份快照直接餵 observe（權限段用）：回傳新增的記錄數與是否丟例外。
async function observeProbe() {
  const N = window.cockpitNotify;
  if (!N || typeof N.observe !== 'function') return { missing: true };
  const s = await (await fetch('/api/state')).json();
  const a = JSON.parse(JSON.stringify(s));
  const b = JSON.parse(JSON.stringify(s));
  for (const r of b.runtimes) for (const w of r.workspaces || []) for (const t of w.tabs || []) for (const p of t.panes || []) if (p.id === 'wJ:p4') p.agent_status = 'blocked';
  const before = window.__notifs ? window.__notifs.length : 0;
  try {
    N.observe(a);
    N.observe(b);
  } catch (e) {
    return { missing: false, threw: String(e), added: (window.__notifs ? window.__notifs.length : 0) - before };
  }
  const added = (window.__notifs ? window.__notifs.slice(before) : []);
  return { missing: false, threw: null, added: added.length, recs: added };
}

// ---------------------------------------------------------------------------
// 共用步驟
// ---------------------------------------------------------------------------

async function load(cdp, url) {
  await cdp.eval('window.__old = true; true');
  await cdp.send('Page.navigate', { url });
  const ok = await cdp.waitFor(
    "window.__old === undefined && document.readyState === 'complete' && document.querySelectorAll('#app .pane-row').length >= 1 && document.getElementById('version').textContent !== ''",
    10000
  );
  pre(ok, `載入 ${url} 並畫出第一份投影`);
  await sleep(300);
  return ok;
}

// 清空本機儲存、寫入記錄器設定，再重新載入（新的頁面＝新的比對基準）。
async function reset(cdp, url, cfg) {
  await load(cdp, url);
  await cdp.eval(`window.__setNotifyCfg(${J(cfg)}); window.__lsClear(); true`);
  await load(cdp, url);
}

async function newNotifs(cdp, mark) {
  const all = await cdp.eval(notifsExpr);
  return all.slice(mark);
}

const exceptionsSince = (cdp, t) => cdp.exceptions.filter((e) => e.at >= t);

async function tabUntil(cdp, predicateExpr, max = 80) {
  for (let i = 0; i < max; i++) {
    await cdp.press('Tab');
    if (await cdp.eval(`(() => { const a = document.activeElement; return !!a && a !== document.body && (${predicateExpr})(a); })()`)) return i + 1;
  }
  return 0;
}

async function openByMouse(cdp) {
  const clicked = await cdp.click(BELL);
  if (!clicked) return false;
  return cdp.waitFor(panelOpenExpr, 2000);
}

async function tabToKind(cdp, kind) {
  return tabUntil(cdp, `(a) => a.dataset && a.dataset.notifyKind === ${J(kind)}`, 12);
}

function sameJson(a, b) {
  return J(a) === J(b);
}

// 一則通知記錄的內容是否等於期望（title／body／tag 精確比對，renotify 必須為 true）。
// 沒有通知時四條都記 FAIL（紅綠兩種跑法的斷言條數一致）。
function checkNotif(rec, exp, where) {
  const r = rec || {};
  const none = rec ? '' : '；沒有通知可檢查';
  check(!!rec && r.title === exp.title, `${where}：標題為「${exp.title}」（實際 ${J(r.title)}${none}）`);
  check(!!rec && r.body === exp.body, `${where}：內文為「${exp.body}」（實際 ${J(r.body)}${none}）`);
  check(!!rec && r.tag === exp.tag, `${where}：識別標籤為 ${exp.tag}（實際 ${J(r.tag)}${none}）`);
  check(!!rec && r.renotify === true, `${where}：renotify 為 true（同標籤取代時仍重新提醒；實際 ${J(r.renotify)}${none}）`);
}

// ---------------------------------------------------------------------------
// N 段：通知事件與呈現（ui_preview 一）
// ---------------------------------------------------------------------------

const N_TRANSITIONS = [
  '300:pane:win/wJ:p3=blocked', // 第一份狀態就有 blocked pane（task ops-2 在 fixture 已是 failed）
  '6000:pane:win/wJ:p1=blocked',
  '8000:pane:win/wJ:p4=done',
  '8000:task:cockpit/be-1=completed',
  '10000:pane:win/wJ:p3=working',
  '10500:pane:win/wJ:p3=blocked',
  '12000:pane:win/wJ:p1=working',
  '12500:pane:win/wJ:p1=blocked',
  '14000:pane:win/wJ:p5=blocked',
];
const N_VANISH = 'wJ:p5=15500';

async function partN(cdp, port) {
  log('=== N 段：通知事件與呈現 ===');
  startServer.vanish = N_VANISH;
  const srv = startServer(port, { transitions: N_TRANSITIONS });
  startServer.vanish = null;
  try {
    if (!pre(await waitUp(port), `ui_preview（N 段）在 10 秒內開始回應；stderr：${srv.stderr().slice(0, 200)}`)) return;
    const url = `http://127.0.0.1:${port}/`;
    // 等第一個轉換（p3 blocked）套用後才載入，讓頁面的第一份狀態就含 blocked pane。
    await waitServer(port, paneIs('win', 'wJ:p3', 'blocked'), 'p3 blocked（載入前）', 0);
    const t0 = Date.now();
    await reset(cdp, url, { permission: 'granted', focused: false, visibility: null, unsupported: false, storageThrows: false });

    log('--- N1 第一份狀態不通知 ---');
    const first = await cdp.eval(`({ p3: (document.querySelector('#app .pane-row[data-pane="wJ:p3"] .status') || {}).textContent || null })`);
    pre(first.p3 === 'blocked', `第一份狀態中 pane wJ:p3 為 blocked（畫面讀到 ${J(first.p3)}）、task ops-2 為 failed（fixture）`);
    await sleep(1500);
    const n1 = await cdp.eval(notifCount);
    check((await cdp.eval(moduleLoaded)) && n1 === 0, `N1 第一份狀態不通知：通知模組已載入（window.cockpitNotify.observe 存在），且載入後 1.5 秒內 0 則通知（實際 ${n1}）`);
    pre(Date.now() - srv.spawnAt < 5800, `N 段前置在第一個轉換（6000 ms）之前完成（實際 ${Date.now() - srv.spawnAt} ms）`);
    pre(exceptionsSince(cdp, t0).length === 0, `N 段載入後沒有未捕捉例外（實際 ${J(exceptionsSince(cdp, t0))}）`);

    log('--- N2 agent 卡住（標題、內文、tag、renotify；多個綁定去重）---');
    let mark = await cdp.eval(notifCount);
    await waitServer(port, paneIs('win', 'wJ:p1', 'blocked'), 'p1 blocked');
    await sleep(800); // 之後多次推送 p1 仍是 blocked，不得重複通知
    let got = await newNotifs(cdp, mark);
    check(got.length === 1, `N2：wJ:p1 由 working 變 blocked 後恰好一則通知（之後持續 blocked 的推送不重發；實際 ${got.length} 則：${J(got)}）`);
    // 綁定 wJ:p1 的 workstream：cockpit/be「Backend」、p/backend「Backend」、p/undeclared「Undeclared」。
    checkNotif(got[0], { title: 'agent 卡住', body: 'win / wJ:p1（Backend、Undeclared）', tag: 'cockpit:blocked:win/wJ:p1' }, 'N2 agent 卡住');
    check(!!got[0] && got[0].focused === false, 'N2：發出當下視窗不在前景（記錄器 hasFocus=false）');

    log('--- N3 預設關閉的類別（done、completed）---');
    mark = await cdp.eval(notifCount);
    await waitServer(port, (s) => paneIs('win', 'wJ:p4', 'done')(s) && taskIs('cockpit', 'be-1', 'completed')(s), 'p4 done、be-1 completed');
    got = await newNotifs(cdp, mark);
    check((await cdp.eval(moduleLoaded)) && got.length === 0, `N3 預設關閉的類別：模組已載入，且 wJ:p4 working→done、be-1 running→completed 不產生通知（實際 ${got.length} 則：${J(got)}）`);

    log('--- N4 另一個 pane 卡住（單一綁定；模糊綁定不算）---');
    mark = await cdp.eval(notifCount);
    await waitServer(port, paneIs('win', 'wJ:p3', 'working'), 'p3 working', 200);
    await waitServer(port, paneIs('win', 'wJ:p3', 'blocked'), 'p3 blocked');
    got = await newNotifs(cdp, mark);
    check(got.length === 1, `N4：wJ:p3 working→blocked 恰好一則通知（實際 ${got.length}）`);
    // qa「QA」以覆蓋綁定 wJ:p3；p/frontend 是 ambiguous（候選含 wJ:p3），不算綁定。
    checkNotif(got[0], { title: 'agent 卡住', body: 'win / wJ:p3（QA）', tag: 'cockpit:blocked:win/wJ:p3' }, 'N4');

    log('--- N5 同一個 pane 再次卡住：同標籤取代、renotify ---');
    mark = await cdp.eval(notifCount);
    await waitServer(port, paneIs('win', 'wJ:p1', 'working'), 'p1 working', 200);
    await waitServer(port, paneIs('win', 'wJ:p1', 'blocked'), 'p1 再次 blocked');
    got = await newNotifs(cdp, mark);
    check(got.length === 1, `N5：wJ:p1 再次 blocked 恰好一則新通知（實際 ${got.length}）`);
    checkNotif(got[0], { title: 'agent 卡住', body: 'win / wJ:p1（Backend、Undeclared）', tag: 'cockpit:blocked:win/wJ:p1' }, 'N5 取代舊通知（同 tag）');

    log('--- N6 沒有綁定的 pane ---');
    mark = await cdp.eval(notifCount);
    await waitServer(port, paneIs('win', 'wJ:p5', 'blocked'), 'p5 blocked');
    got = await newNotifs(cdp, mark);
    checkNotif(got[0], { title: 'agent 卡住', body: 'win / wJ:p5', tag: 'cockpit:blocked:win/wJ:p5' }, 'N6 無綁定 pane');
    await waitServer(port, (s) => findPane(s, 'win', 'wJ:p5') === null, 'p5 消失');

    const all = await cdp.eval(notifsExpr);
    check(all.length > 0 && all.every((r) => r.renotify === true), `N：所有通知都帶 renotify: true（共 ${all.length} 則）`);
    check(
      all.length > 0 && all.every((r) => /^cockpit:(blocked|done):[^/]+\/.+$/.test(r.tag) || /^cockpit:(failed|completed):[^/]+\/.+$/.test(r.tag) || r.tag === 'cockpit:summary'),
      `N：所有通知的 tag 符合 cockpit:<類別>:<runtime>/<pane>（實際 ${J(all.map((r) => r.tag))}）`
    );

    log('--- N7 點通知帶出 pane ---');
    let sel = await cdp.eval(selectionExpr);
    pre(sel.rows.length === 0, `點通知前沒有選定的 pane（實際 ${J(sel)}）`);
    let c = await cdp.fn(clickNotification, 'cockpit:blocked:win/wJ:p1');
    check(c.found, 'N7：找得到 wJ:p1 的 blocked 通知可點');
    check(c.found && c.focusDelta === 1, `N7：點通知時呼叫 window.focus()（帶到前景；實際 ${J(c.focusDelta)} 次）`);
    check(c.found && c.closed === true, 'N7：點通知後關閉該通知（close()）');
    await sleep(500);
    sel = await cdp.eval(selectionExpr);
    check(sameJson(sel.rows, ['win/wJ:p1']), `N7：右欄 wJ:p1 列出現選定標示（實際 ${J(sel.rows)}）`);
    check(sel.title === 'win / wJ:p1', `N7：Live Output 顯示 win / wJ:p1（實際 ${J(sel.title)}）`);
    await sleep(800);
    sel = await cdp.eval(selectionExpr);
    check(sameJson(sel.rows, ['win/wJ:p1']) && sel.title === 'win / wJ:p1', `N7：其後的背景重畫後選取仍在（實際 ${J(sel)}）`);

    log('--- N8 pane 已不在最新狀態中時點通知：只帶到前景 ---');
    c = await cdp.fn(clickNotification, 'cockpit:blocked:win/wJ:p5');
    check(c.found && c.focusDelta === 1 && c.closed === true, `N8：點已消失 pane（wJ:p5）的通知仍呼叫 window.focus() 並關閉（實際 ${J(c)}）`);
    await sleep(500);
    sel = await cdp.eval(selectionExpr);
    check(c.found && sameJson(sel.rows, ['win/wJ:p1']) && sel.title === 'win / wJ:p1', `N8：選取不變、仍是 wJ:p1（實際 ${J(sel)}）`);

    log('--- N9 改綁模式中點通知：只帶到前景 ---');
    const rebindClicked = await cdp.click('#app [data-action="rebind"]');
    const inRebind = rebindClicked && (await cdp.waitFor("!!document.querySelector('#app [data-action=\"rebind-cancel\"]')", 2000));
    pre(inRebind, '以滑鼠按「改綁」進入改綁模式');
    c = await cdp.fn(clickNotification, 'cockpit:blocked:win/wJ:p3');
    check(c.found && c.focusDelta === 1 && c.closed === true, `N9：改綁模式中點 wJ:p3 的通知仍呼叫 window.focus() 並關閉（實際 ${J(c)}）`);
    await sleep(500);
    sel = await cdp.eval(selectionExpr);
    const stillRebind = await cdp.eval("!!document.querySelector('#app [data-action=\"rebind-cancel\"]')");
    check(c.found && sameJson(sel.rows, ['win/wJ:p1']) && sel.title === 'win / wJ:p1', `N9：改綁模式中點通知後選取仍是 wJ:p1（實際 ${J(sel)}）`);
    check(c.found && stillRebind, 'N9：點通知不離開改綁模式');
    if (inRebind) await cdp.click('#app [data-action="rebind-cancel"]');
    await cdp.waitFor("!document.querySelector('#app [data-action=\"rebind-cancel\"]')", 2000);

    log('--- N10 exited 的 pane：點通知與點 pane 列一樣不選取（修正波 3.6 M3）---');
    // fixture 的 wJ:p2 是 exited（仍在投影中），pane 列不可點；selectPane 是點通知的入口（design D7）。
    const p2 = await cdp.eval(
      "(() => { const r = document.querySelector('#app .pane-row[data-runtime=\"win\"][data-pane=\"wJ:p2\"]'); return r ? { exited: r.classList.contains('exited'), action: r.getAttribute('data-action') } : null; })()"
    );
    pre(!!p2 && p2.exited && p2.action === null, `wJ:p2 在畫面上是 exited、pane 列沒有 data-action（實際 ${J(p2)}）`);
    sel = await cdp.eval(selectionExpr);
    pre(sameJson(sel.rows, ['win/wJ:p1']), `N10 之前選定 wJ:p1（實際 ${J(sel)}）`);
    const r10 = await cdp.eval("window.cockpitActions && typeof window.cockpitActions.selectPane === 'function' ? window.cockpitActions.selectPane('win', 'wJ:p2') : 'missing'");
    await sleep(400);
    sel = await cdp.eval(selectionExpr);
    check(r10 === false, `N10：selectPane 對 exited 的 pane（wJ:p2）回 false（實際 ${J(r10)}）`);
    check(sameJson(sel.rows, ['win/wJ:p1']) && sel.title === 'win / wJ:p1', `N10：點 exited pane 的通知不改變選取，仍是 wJ:p1（實際 ${J(sel)}）`);

    log('--- N11 點通知選定的 pane 列捲進視野（修正波 3.6 M8；窄版 700×500）---');
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 700, height: 500, deviceScaleFactor: 1, mobile: false });
    try {
      await sleep(500);
      const p3Rect = "(() => { const r = document.querySelector('#app .pane-row[data-runtime=\"win\"][data-pane=\"wJ:p3\"]'); if (!r) return null; const b = r.getBoundingClientRect(); return { top: b.top, bottom: b.bottom, vh: window.innerHeight }; })()";
      await cdp.eval('window.scrollTo(0, 0); true');
      await sleep(200);
      let rr = await cdp.eval(p3Rect);
      if (rr && rr.top < rr.vh && rr.bottom > 0) {
        await cdp.eval('window.scrollTo(0, document.scrollingElement.scrollHeight); true');
        await sleep(200);
        rr = await cdp.eval(p3Rect);
      }
      pre(!!rr && (rr.bottom <= 0 || rr.top >= rr.vh), `點通知前 wJ:p3 列在視窗外（實際 ${J(rr)}）`);
      c = await cdp.fn(clickNotification, 'cockpit:blocked:win/wJ:p3');
      await sleep(500);
      sel = await cdp.eval(selectionExpr);
      pre(c.found && sameJson(sel.rows, ['win/wJ:p3']), `點 wJ:p3 的通知後選定 wJ:p3（實際 ${J(sel)}）`);
      rr = await cdp.eval(p3Rect);
      // 容許 1px：列高與捲動位置有次像素，scrollIntoView({block: 'nearest'}) 貼齊下緣時可能差零點幾 px。
      check(!!rr && rr.top >= -1 && rr.bottom <= rr.vh + 1, `N11：點通知後選定的 wJ:p3 列完整出現在視窗內（容許 1px 次像素；實際 ${J(rr)}）`);
    } finally {
      await cdp.send('Emulation.clearDeviceMetricsOverride');
    }
  } finally {
    await cdp.send('Page.navigate', { url: 'about:blank' });
    await sleep(300);
    killTree(srv.proc, 'ui_preview（N 段）');
  }
}

// ---------------------------------------------------------------------------
// M 段：前景、合併、開啟後的類別、task 通知、重連（ui_preview 二＋重啟）
// ---------------------------------------------------------------------------

const FOUR_FAILED = ['cockpit/be-1', 'cockpit/docs-1', 'cockpit/release-1', 'cockpit/be-2'];
const FOUR_TITLES = ['投影擴充', 'README', '發布準備', '介面草稿'];
const M_TRANSITIONS = [
  '8000:pane:win/wJ:p1=blocked',
  '10000:pane:win/wJ:p3=blocked',
  ...FOUR_FAILED.map((t) => `12000:task:${t}=failed`),
  '14000:task:p/backend-1=completed',
  '16000:pane:win/wJ:p4=done',
  '18000:task:p/frontend-1=failed',
  // M9 重連前置：把 fixture 中原本 failed 的 cockpit/ops-2 改成 running（failed→running 不通知）。
  // 重啟後的 ui_preview 從 fixture 開始，第一份狀態中 ops-2 就是 failed，與時序無關。
  '19000:task:cockpit/ops-2=running',
];

async function partM(cdp, port) {
  log('=== M 段：前景不打擾、合併、開啟後的類別、重連 ===');
  let srv = startServer(port, { transitions: M_TRANSITIONS });
  try {
    if (!pre(await waitUp(port), `ui_preview（M 段）在 10 秒內開始回應；stderr：${srv.stderr().slice(0, 200)}`)) return;
    const url = `http://127.0.0.1:${port}/`;
    await reset(cdp, url, { permission: 'granted', focused: false, visibility: null, unsupported: false, storageThrows: false });

    log('--- M1 以設定面板開啟 done 與 completed（滑鼠開、鍵盤切）---');
    const opened = await openByMouse(cdp);
    check(opened, 'M1：按鈴鐺開啟設定面板');
    let ok = false;
    if (opened) {
      const a = await tabToKind(cdp, 'done');
      if (a) await cdp.press('Space');
      const b = await tabToKind(cdp, 'completed');
      if (b) await cdp.press('Space');
      const info = await cdp.eval(panelExpr);
      ok = !!a && !!b && info.state.done === true && info.state.completed === true;
      await cdp.press('Escape');
    }
    check(ok, 'M1：在面板以鍵盤開啟 done 與 completed 兩個開關');
    pre(Date.now() - srv.spawnAt < 7800, `M 段前置在第一個轉換（8000 ms）之前完成（實際 ${Date.now() - srv.spawnAt} ms）`);

    log('--- M2 正在看畫面時不打擾 ---');
    await cdp.eval("window.__setNotifyCfg({ focused: true, visibility: 'visible' }); true");
    let mark = await cdp.eval(notifCount);
    await waitServer(port, paneIs('win', 'wJ:p1', 'blocked'), 'p1 blocked（前景）');
    let got = await newNotifs(cdp, mark);
    check((await cdp.eval(moduleLoaded)) && got.length === 0, `M2 正在看畫面時不打擾：模組已載入，頁面可見且有焦點時 wJ:p1 變 blocked 不發通知（實際 ${got.length} 則）`);

    log('--- M3 頁面不可見（例如最小化）時照發 ---');
    await cdp.eval("window.__setNotifyCfg({ focused: true, visibility: 'hidden' }); true");
    mark = await cdp.eval(notifCount);
    await waitServer(port, paneIs('win', 'wJ:p3', 'blocked'), 'p3 blocked（不可見）');
    got = await newNotifs(cdp, mark);
    check(got.length === 1, `M3：visibilityState=hidden 時 wJ:p3 變 blocked 照發一則（實際 ${got.length}）`);
    checkNotif(got[0], { title: 'agent 卡住', body: 'win / wJ:p3（QA）', tag: 'cockpit:blocked:win/wJ:p3' }, 'M3');
    await cdp.eval('window.__setNotifyCfg({ focused: false, visibility: null }); true');

    log('--- M4 合併通知（同一份狀態 4 個 task 變 failed）---');
    mark = await cdp.eval(notifCount);
    await waitServer(port, (s) => FOUR_FAILED.every((x) => taskIs(x.split('/')[0], x.split('/')[1], 'failed')(s)), '四個 task failed', 1200);
    got = await newNotifs(cdp, mark);
    check(got.length === 1, `M4：只發出一則通知（實際 ${got.length} 則：${J(got.map((r) => r.title))}）`);
    const sum = got[0] || {};
    const listed = FOUR_TITLES.filter((t) => (sum.body || '').includes(t));
    check(sum.title === 'Cockpit：4 件事需要注意', `M4：標題為「Cockpit：4 件事需要注意」（實際 ${J(sum.title)}）`);
    check(listed.length === 3, `M4：內文列出前 3 件（4 個 task 標題中恰好出現 3 個；實際 ${J(listed)}，內文 ${J(sum.body)}）`);
    check(typeof sum.body === 'string' && sum.body.trimEnd().endsWith('…'), `M4：內文以「…」表示其後還有（實際 ${J(sum.body)}）`);
    check(sum.tag === 'cockpit:summary', `M4：合併通知標籤為 cockpit:summary（實際 ${J(sum.tag)}）`);
    check(sum.renotify === true, `M4：合併通知 renotify 為 true（實際 ${J(sum.renotify)}）`);

    log('--- M5 開啟後的類別：task completed ---');
    mark = await cdp.eval(notifCount);
    await waitServer(port, taskIs('p', 'backend-1', 'completed'), 'p/backend-1 completed');
    got = await newNotifs(cdp, mark);
    check(got.length === 1, `M5：開啟 completed 後 task 由 running 變 completed 發一則（實際 ${got.length}）`);
    checkNotif(got[0], { title: 'task completed', body: 'Scenario D Demo：後端實作', tag: 'cockpit:completed:p/backend-1' }, 'M5');

    log('--- M6 開啟後的類別：agent done ---');
    mark = await cdp.eval(notifCount);
    await waitServer(port, paneIs('win', 'wJ:p4', 'done'), 'p4 done');
    got = await newNotifs(cdp, mark);
    check(got.length === 1, `M6：開啟 done 後 pane 由 idle 變 done 發一則（實際 ${got.length}）`);
    checkNotif(got[0], { title: 'agent 停下等你看', body: 'win / wJ:p4', tag: 'cockpit:done:win/wJ:p4' }, 'M6');

    log('--- M7 單一 task failed ---');
    mark = await cdp.eval(notifCount);
    await waitServer(port, taskIs('p', 'frontend-1', 'failed'), 'p/frontend-1 failed');
    got = await newNotifs(cdp, mark);
    check(got.length === 1, `M7：單一 task 變 failed 發一則（實際 ${got.length}）`);
    checkNotif(got[0], { title: 'task failed', body: 'Scenario D Demo：前端規劃', tag: 'cockpit:failed:p/frontend-1' }, 'M7');

    log('--- M8 點 task 通知：帶到前景、關閉，不改選取 ---');
    const c = await cdp.fn(clickNotification, 'cockpit:summary');
    check(c.found && c.focusDelta === 1 && c.closed === true, `M8：點合併通知呼叫 window.focus() 並關閉（實際 ${J(c)}）`);
    await sleep(300);
    const sel = await cdp.eval(selectionExpr);
    check(c.found && sel.rows.length === 0, `M8：點 task 通知不選定任何 pane（實際 ${J(sel)}）`);

    log('--- M9 重連後補報（真的停掉 ui_preview 再重啟）---');
    // 做法：斷線前最後一份狀態 ops-2 為 running（上面 19000 ms 的轉換）；重啟的 ui_preview 從 fixture
    // 開始，ops-2 是 failed。重連後第一份狀態就帶著 failed，與重試時序無關（Windows 連到沒人聽的
    // localhost 埠要約 2 秒才失敗，重試時間點無法控制，所以不靠「新 ui_preview 在頁面連上前先套用轉換」）。
    // 保留基準 → running→failed 發一則；若重連時重設基準，這份只當基準、不發——可以分辨。
    // 其餘欄位相對 fixture 的差異（p1／p3 blocked→working／unknown、四個 task failed→原值、
    // p/backend-1 completed→running、wJ:p4 done→idle、p/frontend-1 failed→running）都不產生事件。
    await waitServer(port, taskIs('cockpit', 'ops-2', 'running'), 'cockpit/ops-2 running（斷線前）');
    const before = await apiState(port);
    pre(taskIs('cockpit', 'ops-2', 'running')(before), '斷線前最後一份狀態中 cockpit/ops-2 為 running');
    mark = await cdp.eval(notifCount);
    const wsMark = (await cdp.eval('window.__wsFirst.length')) || 0;
    killTree(srv.proc, 'ui_preview（M 段，停掉造成斷線）');
    const down = await cdp.waitFor("document.getElementById('app').getAttribute('data-channel-state') !== 'connected'", 8000);
    pre(down, '服務停掉後頁面通道離開 connected');
    // 新的 ui_preview 設一條一小時後才套用的無害轉換，只為了停掉 wJ:p1 的輪替。
    srv = startServer(port, { transitions: ['3600000:pane:win/wJ:p4=idle'] });
    pre(await waitUp(port), 'ui_preview 重啟後開始回應');
    const up = await cdp.waitFor("document.getElementById('app').getAttribute('data-channel-state') === 'connected'", 20000);
    pre(up, '重連後頁面通道回到 connected');
    await sleep(1000);
    const firsts = (await cdp.eval('window.__wsFirst')).slice(wsMark);
    pre(firsts.length >= 1 && firsts[0].ops2Status === 'failed', `重連後收到的第一份狀態中 cockpit/ops-2 為 failed（實際 ${J(firsts)}）`);
    got = await newNotifs(cdp, mark);
    check(got.length === 1, `M9 重連後補報：與斷線前最後一份比對，恰好一則通知（實際 ${got.length} 則：${J(got)}）`);
    checkNotif(got[0], { title: 'task failed', body: 'AI Cockpit：上線檢查', tag: 'cockpit:failed:cockpit/ops-2' }, 'M9');
  } finally {
    await cdp.send('Page.navigate', { url: 'about:blank' });
    await sleep(300);
    killTree(srv.proc, 'ui_preview（M 段）');
  }
}

// ---------------------------------------------------------------------------
// S 段：設定面板、鈴鐺、權限、路由（ui_preview 三）；P 段：純函式
// ---------------------------------------------------------------------------

const FAIL_PATH = '/api/projects/cockpit/tasks/be-1/fail';
// 修正波 3.6 M1：慢慢失敗的寫入（1500 ms 後才回 409），在它進行中按鈴鐺。
const SLOW_FAIL_PATH = '/api/projects/cockpit/tasks/docs-1/fail';

async function partS(cdp, port) {
  log('=== S 段：設定面板與鈴鐺 ===');
  // 設 TRANSITIONS（一小時後才套用的無害規則）以停止既有的 wJ:p1 輪替；fail 一律回 409 以產生錯誤訊息。
  const srv = startServer(port, { transitions: ['3600000:pane:win/wJ:p4=idle'], writeRules: [`${FAIL_PATH}=0:409`, `${SLOW_FAIL_PATH}=1500:409`] });
  try {
    if (!pre(await waitUp(port), `ui_preview（S 段）在 10 秒內開始回應；stderr：${srv.stderr().slice(0, 200)}`)) return;
    const url = `http://127.0.0.1:${port}/`;

    log('--- S0 路由 ---');
    const r = await fetch(`http://127.0.0.1:${port}/app/notify.js`);
    const ct = r.headers.get('content-type') || '';
    check(r.status === 200 && /javascript/i.test(ct), `S0：GET /app/notify.js 為 200、content-type 為 JavaScript（實際 ${r.status} ${ct}）`);
    const html = await (await fetch(url)).text();
    const iNotify = html.indexOf('/app/notify.js');
    const iRender = html.indexOf('/app/render.js');
    check(iNotify !== -1 && iNotify < iRender, `S0（design D7）：index.html 在 render.js 之前載入 notify.js（位置 ${iNotify} / ${iRender}）`);

    await reset(cdp, url, { permission: 'granted', focused: false, visibility: null, unsupported: false, storageThrows: false });
    const t0 = Date.now();

    log('--- S1 鈴鐺按鈕跨重畫保留焦點（鍵盤）---');
    const bell = await cdp.eval(`(() => { const b = document.querySelector(${J(BELL)}); return b ? { tag: b.tagName, inTopbar: !!b.closest('[data-region="topbar"]') } : null; })()`);
    check(!!bell && bell.tag === 'BUTTON' && bell.inTopbar, `S1：頂列有 data-action="notify-settings" 的鈴鐺 <button>（實際 ${J(bell)}）`);
    check((await cdp.eval(moduleLoaded)) && !(await cdp.eval(panelOpenExpr)), 'S1：通知模組已載入，且載入後設定面板未開啟');
    const hops = await tabUntil(cdp, `(a) => a.matches(${J(BELL)})`, 60);
    check(hops > 0, `S1：以 Tab 把焦點移到鈴鐺（Tab ${hops} 次）`);
    if (hops > 0) {
      await cdp.eval(`window.__bell = document.querySelector(${J(BELL)}); true`);
      const f0 = await cdp.eval(focusExpr);
      check(f0.onBell && f0.fv, `S1：Tab 到鈴鐺時外框可見（:focus-visible；實際 ${J(f0)}）`);
      const v0 = await cdp.eval(versionOf);
      await sleep(1000);
      const v1 = await cdp.eval(versionOf);
      pre(v1 >= v0 + 3, `這 1 秒內收到至少 3 份新投影、整頁重畫數次（version ${v0} -> ${v1}）`);
      pre(await cdp.eval(`window.__bell !== document.querySelector(${J(BELL)})`), '鈴鐺已被重畫換成新節點');
      const f1 = await cdp.eval(focusExpr);
      check(f1.onBell, `S1：1 秒後焦點仍在（新的）鈴鐺上（實際 ${J(f1)}）`);
      check(f1.fv, `S1：1 秒後焦點外框仍可見（實際 ${J(f1)}）`);
    }

    log('--- S2 按 Enter 開啟：焦點到第一個開關、外框、結構 ---');
    if (hops > 0) await cdp.press('Enter');
    await cdp.waitFor(panelOpenExpr, 2000);
    let p = await cdp.eval(panelExpr);
    check(hops > 0 && p.open, 'S2：在鈴鐺上按 Enter 開啟通知設定面板');
    check(p.exists && !p.insideApp && p.inBody, `S2（design D8）：面板是 body 底下、#app 之外的節點（實際 exists=${p.exists} insideApp=${p.insideApp}）`);
    check(sameJson(p.kinds, KINDS), `S2：四個開關依序為 ${J(KINDS)}（實際 ${J(p.kinds)}）`);
    check(p.focusIsFirstToggle, `S2：開啟時焦點移到第一個開關（實際焦點類別 ${J(p.focusKind)}）`);
    const ring = await cdp.fn(focusRing);
    check(p.focusIsFirstToggle && !!ring && ring.fv && ring.ok, `S2：第一個開關匹配 :focus-visible 且焦點外框可見（實際 ${J(ring)}）`);

    log('--- S3 預設值、標籤、禁字 ---');
    check(p.open && sameJson(p.state, DEFAULTS), `S3 預設值：blocked、failed 開，done、completed 關（實際 ${J(p.state)}）`);
    const names = await cdp.fn(toggleNames);
    for (const k of KINDS) {
      check(!!names && typeof names[k] === 'string' && names[k].includes(LABELS[k]), `S3：${k} 開關的可及名稱含「${LABELS[k]}」（實際 ${J(names && names[k])}）`);
    }
    const order = KINDS.map((k) => (p.text || '').indexOf(LABELS[k]));
    check(p.exists && order.every((i) => i >= 0) && order.every((i, j) => j === 0 || i > order[j - 1]), `S3：面板文字依序出現四個標籤（位置 ${J(order)}）`);
    check(p.exists && /[\u4e00-\u9fff]/.test(p.text), 'S3：面板含中文說明');
    check(p.exists && !p.text.includes('完成') && !p.attrText.includes('完成'), `S3：面板文字與 title／aria-label 不出現「完成」（文字 ${J((p.text || '').slice(0, 200))}）`);
    check(p.exists && p.text.includes('已允許'), `S3：權限已允許時面板顯示「已允許」（文字 ${J((p.text || '').slice(0, 200))}）`);

    log('--- S4 面板文字對比、減少動態 ---');
    const cr = await cdp.fn(contrastProbe);
    const low = (cr || []).filter((x) => x.ratio < 4.5);
    check(!!cr && cr.length >= 4 && low.length === 0, `S4：面板內 ${cr ? cr.length : 0} 段可見文字的對比皆 ≥ 4.5:1（不足：${J(low)}；最低 ${cr && cr.length ? Math.min(...cr.map((x) => x.ratio)) : 'n/a'}）`);
    await cdp.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    const mo = await cdp.fn(motionProbe);
    check(Array.isArray(mo) && mo.length === 0, `S4：減少動態設定下面板沒有動畫與 transition（實際 ${J(mo)}）`);
    await cdp.send('Emulation.setEmulatedMedia', { features: [] });

    log('--- S5 重畫不關閉面板、開關狀態不變 ---');
    const toDone = p.open ? await tabToKind(cdp, 'done') : 0;
    if (toDone) await cdp.press('Space');
    p = await cdp.eval(panelExpr);
    check(p.open && p.state.done === true, `S5：以 Space 開啟 done（實際 ${J(p.state)}）`);
    let stored = await cdp.eval(`window.__lsGet(${J(STORAGE_KEY)})`);
    check(stored !== null && sameJson(safeParse(stored), { blocked: true, done: true, failed: true, completed: false }), `S5：變更立即寫入本機儲存 ${STORAGE_KEY}（實際 ${J(stored)}）`);
    await cdp.eval("window.__panelNode = document.getElementById('notify-panel'); true");
    const v0 = await cdp.eval(versionOf);
    await sleep(2000);
    const v1 = await cdp.eval(versionOf);
    pre(v1 >= v0 + 10, `2 秒內收到至少 10 份新投影、整頁重畫多次（version ${v0} -> ${v1}）`);
    p = await cdp.eval(panelExpr);
    check(p.open && (await cdp.eval("window.__panelNode === document.getElementById('notify-panel') && window.__panelNode.isConnected")), 'S5 重畫不關閉面板：面板仍開啟、DOM 節點沒有被換掉');
    check(p.open && sameJson(p.state, { blocked: true, done: true, failed: true, completed: false }), `S5：重畫後開關狀態不變（實際 ${J(p.state)}）`);
    check(p.focusKind === 'done', `S5：重畫後焦點仍在 done 開關（實際 ${J(p.focusKind)}）`);

    log('--- S6 Esc 關閉、焦點回鈴鐺 ---');
    await cdp.press('Escape');
    await sleep(200);
    p = await cdp.eval(panelExpr);
    let f = await cdp.eval(focusExpr);
    check(toDone > 0 && !p.open, 'S6：按 Esc 關閉面板');
    check(toDone > 0 && f.onBell, `S6：關閉後焦點回到鈴鐺（實際 ${J(f)}）`);

    log('--- S7 再按一次鈴鐺關閉（鍵盤開、滑鼠關）---');
    if (f.onBell) await cdp.press('Enter');
    const reopened = await cdp.waitFor(panelOpenExpr, 2000);
    p = await cdp.eval(panelExpr);
    check(reopened && p.focusIsFirstToggle, `S7：在鈴鐺上按 Enter 再開啟，焦點到第一個開關（實際 ${J(p.focusKind)}）`);
    await cdp.click(BELL);
    await sleep(300);
    p = await cdp.eval(panelExpr);
    f = await cdp.eval(focusExpr);
    check(reopened && !p.open, 'S7：再按一次鈴鐺（滑鼠）關閉面板');
    check(reopened && f.onBell, `S7：關閉後焦點回到鈴鐺（實際 ${J(f)}）`);

    log('--- S8 滑鼠開啟後不被同一次點擊關掉；點面板外關閉 ---');
    const opened8 = await openByMouse(cdp);
    await sleep(300);
    p = await cdp.eval(panelExpr);
    check(opened8 && p.open, 'S8：滑鼠按鈴鐺開啟、300 ms 後仍開啟（點外面關閉的判定排除鈴鐺）');
    check(opened8 && p.focusIsFirstToggle, `S8：滑鼠開啟時焦點同樣移到第一個開關（實際 ${J(p.focusKind)}）`);
    await cdp.click('#app-name');
    await sleep(300);
    p = await cdp.eval(panelExpr);
    f = await cdp.eval(focusExpr);
    check(opened8 && !p.open, 'S8：點面板與鈴鐺以外的位置（產品名稱）關閉面板');
    check(opened8 && f.onBell, `S8：焦點原本在面板內，關閉後回到鈴鐺（實際 ${J(f)}）`);

    log('--- S9 設定保留（關 blocked、開 done，重新載入）---');
    // 此時本機儲存已是 done 開（S5）；再關掉 blocked。
    const opened9 = await openByMouse(cdp);
    if (opened9) {
      p = await cdp.eval(panelExpr);
      if (p.focusKind === 'blocked') await cdp.press('Space');
      await cdp.press('Escape');
    }
    stored = await cdp.eval(`window.__lsGet(${J(STORAGE_KEY)})`);
    check(stored !== null && sameJson(safeParse(stored), { blocked: false, done: true, failed: true, completed: false }), `S9：本機儲存為 blocked 關、done 開（實際 ${J(stored)}）`);
    await load(cdp, url);
    const opened9b = await openByMouse(cdp);
    p = await cdp.eval(panelExpr);
    check(opened9b && sameJson(p.state, { blocked: false, done: true, failed: true, completed: false }), `S9 設定保留：重新載入後 blocked 關、done 開（實際 ${J(p.state)}）`);
    if (opened9b) await cdp.press('Escape');

    log('--- S10 本機儲存損毀：使用預設值、不報錯 ---');
    for (const bad of ['{not json', J({ blocked: 'yes', done: 1, failed: null, completed: 'true' })]) {
      await cdp.eval(`window.__lsSet(${J(STORAGE_KEY)}, ${J(bad)}); true`);
      const tl = Date.now();
      await load(cdp, url);
      const o = await openByMouse(cdp);
      p = await cdp.eval(panelExpr);
      check(o && sameJson(p.state, DEFAULTS), `S10：本機儲存為 ${bad} 時使用預設值（實際 ${J(p.state)}）`);
      const ex = exceptionsSince(cdp, tl);
      check(o && ex.length === 0, `S10：本機儲存為 ${bad} 時不報錯（未捕捉例外 ${J(ex)}）`);
      if (o) await cdp.press('Escape');
    }

    log('--- S11 本機儲存不可用：使用預設值、不報錯 ---');
    await cdp.eval('window.__setNotifyCfg({ storageThrows: true }); true');
    let tl = Date.now();
    await load(cdp, url);
    pre(await cdp.eval('(() => { try { void window.localStorage; return false; } catch (e) { return true; } })()'), '模擬成功：存取 window.localStorage 會丟例外');
    let o = await openByMouse(cdp);
    p = await cdp.eval(panelExpr);
    check(o && sameJson(p.state, DEFAULTS), `S11：本機儲存不可用時使用預設值（實際 ${J(p.state)}）`);
    if (o && p.focusKind === 'blocked') await cdp.press('Space');
    await sleep(200);
    p = await cdp.eval(panelExpr);
    check(o && p.state.blocked === false, `S11：本機儲存不可用時切換開關仍立即生效（實際 ${J(p.state)}）`);
    let ex = exceptionsSince(cdp, tl);
    check(o && ex.length === 0, `S11：本機儲存不可用時載入與切換都不報錯（未捕捉例外 ${J(ex)}）`);
    if (o) await cdp.press('Escape');
    await cdp.eval('window.__setNotifyCfg({ storageThrows: false }); window.__lsClear(); true');

    log('--- S12 請求權限（尚未決定 → 按「允許通知」）---');
    await cdp.eval("window.__setNotifyCfg({ permission: 'default', requestResult: 'granted' }); true");
    tl = Date.now();
    await load(cdp, url);
    let ob = await cdp.fn(observeProbe);
    check(!ob.missing && !ob.threw && ob.added === 0, `S12：權限尚未決定時不發出通知、不報錯（實際 ${J(ob)}）`);
    check(!ob.missing && (await cdp.eval('window.__permRequests')) === 0, 'S12：載入與收到狀態時不主動請求權限');
    o = await openByMouse(cdp);
    p = await cdp.eval(panelExpr);
    check(o && p.buttons.includes('允許通知'), `S12：面板顯示「允許通知」按鈕（實際按鈕 ${J(p.buttons)}）`);
    const clickedAllow = o && (await cdp.clickExpr("Array.from(document.querySelectorAll('#notify-panel button')).find((b) => b.textContent.trim() === '允許通知')"));
    await sleep(400);
    check(clickedAllow && (await cdp.eval('window.__permRequests')) === 1, `S12：按下「允許通知」才向瀏覽器請求權限一次（實際 ${await cdp.eval('window.__permRequests')} 次）`);
    const granted = await cdp.waitFor(`(${panelInfo.toString()})().text.includes('已允許')`, 2000);
    p = await cdp.eval(panelExpr);
    check(clickedAllow && granted && !p.buttons.includes('允許通知'), `S12：允許後面板顯示已允許、「允許通知」按鈕消失（文字 ${J((p.text || '').slice(0, 160))}）`);
    check(o && exceptionsSince(cdp, tl).length === 0, `S12：過程不報錯（未捕捉例外 ${J(exceptionsSince(cdp, tl))}）`);
    if (o) await cdp.press('Escape');

    log('--- S13 權限已封鎖 ---');
    await cdp.eval("window.__setNotifyCfg({ permission: 'denied' }); true");
    tl = Date.now();
    await load(cdp, url);
    ob = await cdp.fn(observeProbe);
    check(!ob.missing && !ob.threw && ob.added === 0, `S13：權限已封鎖時不發出通知、不報錯（實際 ${J(ob)}）`);
    o = await openByMouse(cdp);
    p = await cdp.eval(panelExpr);
    check(o && p.text.includes('網站設定') && !p.buttons.includes('允許通知'), `S13：面板顯示到瀏覽器網站設定解除的說明、沒有「允許通知」按鈕（文字 ${J((p.text || '').slice(0, 200))}）`);
    check(o && (await cdp.eval('window.__permRequests')) === 0, 'S13：已封鎖時不請求權限');
    check(o && exceptionsSince(cdp, tl).length === 0, `S13：不報錯（未捕捉例外 ${J(exceptionsSince(cdp, tl))}）`);
    if (o) await cdp.press('Escape');

    log('--- S14 瀏覽器不支援通知 ---');
    await cdp.eval('window.__setNotifyCfg({ unsupported: true }); true');
    tl = Date.now();
    await load(cdp, url);
    pre(await cdp.eval("!('Notification' in window)"), '模擬成功：window.Notification 不存在');
    ob = await cdp.fn(observeProbe);
    check(!ob.missing && !ob.threw, `S14：不支援時收到狀態變化不報錯（實際 ${J(ob)}）`);
    o = await openByMouse(cdp);
    p = await cdp.eval(panelExpr);
    check(o && p.text.includes('不支援') && !p.buttons.includes('允許通知'), `S14：面板顯示不支援的說明、沒有「允許通知」按鈕（文字 ${J((p.text || '').slice(0, 200))}）`);
    check(o && exceptionsSince(cdp, tl).length === 0, `S14：不報錯（未捕捉例外 ${J(exceptionsSince(cdp, tl))}）`);
    if (o) await cdp.press('Escape');

    log('--- S15 已允許（對照組：同樣的 observe 會發出一則）---');
    await cdp.eval("window.__setNotifyCfg({ unsupported: false, permission: 'granted' }); true");
    await load(cdp, url);
    ob = await cdp.fn(observeProbe);
    check(!ob.missing && !ob.threw && ob.added === 1, `S15：權限已允許時 observe 兩份快照（wJ:p4 idle→blocked）發出一則（實際 ${J(ob)}）`);
    checkNotif(ob.recs && ob.added === 1 ? ob.recs[0] : null,{ title: 'agent 卡住', body: 'win / wJ:p4', tag: 'cockpit:blocked:win/wJ:p4' }, 'S15');

    log('--- S16 鈴鐺不影響畫面操作的錯誤訊息 ---');
    const clickedFail = await cdp.click('#app [data-action="fail"][data-project="cockpit"][data-task="be-1"]');
    const hasErr = clickedFail && (await cdp.waitFor("!!document.querySelector('#app .error-banner')", 3000));
    pre(hasErr, `按 be-1 的「Failed」（ui_preview 回 409）後出現錯誤訊息`);
    const errText = hasErr ? await cdp.eval("document.querySelector('#app .error-banner').textContent") : null;
    const o16 = await openByMouse(cdp);
    let errNow = await cdp.eval("(document.querySelector('#app .error-banner') || {}).textContent || null");
    check(o16 && hasErr && errNow === errText, `S16：按鈴鐺開啟設定面板後錯誤訊息仍在（實際 ${J(errNow)}）`);
    if (o16) await cdp.click(BELL);
    await sleep(300);
    p = await cdp.eval(panelExpr);
    errNow = await cdp.eval("(document.querySelector('#app .error-banner') || {}).textContent || null");
    check(o16 && !p.open && hasErr && errNow === errText, `S16 鈴鐺不影響畫面操作的錯誤訊息：開啟再關閉面板後錯誤訊息仍在（實際 ${J(errNow)}）`);
    // 之後一段背景重畫，錯誤訊息仍在（鈴鐺沒有讓錯誤進入「已過期」）。
    await sleep(600);
    errNow = await cdp.eval("(document.querySelector('#app .error-banner') || {}).textContent || null");
    check(o16 && hasErr && errNow === errText, `S16：其後背景重畫錯誤訊息仍在（實際 ${J(errNow)}）`);
    if (hasErr) await cdp.click('#app .error-banner [data-action="error-dismiss"]');

    log('--- S17 鈴鐺不離開改綁模式 ---');
    const rb = await cdp.click('#app [data-action="rebind"]');
    const inRebind = rb && (await cdp.waitFor("!!document.querySelector('#app [data-action=\"rebind-cancel\"]')", 2000));
    pre(inRebind, '以滑鼠按「改綁」進入改綁模式');
    const o17 = await openByMouse(cdp);
    if (o17) await cdp.click(BELL);
    await sleep(300);
    check(o17 && inRebind && (await cdp.eval("!!document.querySelector('#app [data-action=\"rebind-cancel\"]')")), 'S17：按鈴鐺開啟再關閉面板後仍在改綁模式');
    if (inRebind) await cdp.click('#app [data-action="rebind-cancel"]');
    await cdp.waitFor("!document.querySelector('#app [data-action=\"rebind-cancel\"]')", 2000);

    // ---- 以下為修正波（3.5 採納項、3.6 M1、M2、M4、M5、M9）新增 ----
    const mouseAway = () => cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 5, y: 600 });

    log('--- S18 鈴鐺 aria-expanded 與按下狀態、開關 accent-color（3.5 採納）---');
    let bp = await cdp.eval(bellProbeExpr);
    check(!!bp && bp.expanded === 'false', `S18：面板關閉時鈴鐺 aria-expanded="false"（實際 ${J(bp && bp.expanded)}）`);
    const closedBorder = bp ? bp.border : null;
    const o18 = await openByMouse(cdp);
    await mouseAway();
    await sleep(200);
    bp = await cdp.eval(bellProbeExpr);
    check(o18 && bp.expanded === 'true', `S18：面板開啟時鈴鐺 aria-expanded="true"（實際 ${J(bp.expanded)}）`);
    check(o18 && bp.border === bp.accent && bp.border !== closedBorder, `S18：面板開啟時鈴鐺呈現按下狀態（框線為 --accent ${bp.accent}，關閉時 ${closedBorder}；實際 ${bp.border}）`);
    check(o18 && bp.boxes.length === 4 && bp.boxes.every((c) => c === bp.accent), `S18：四個開關的 accent-color 為 --accent（實際 ${J(bp.boxes)}）`);
    await cdp.eval(`window.__bell18 = document.querySelector(${J(BELL)}); true`);
    await sleep(1000);
    pre(await cdp.eval(`window.__bell18 !== document.querySelector(${J(BELL)})`), '面板開著的這 1 秒內鈴鐺已被重畫換成新節點');
    bp = await cdp.eval(bellProbeExpr);
    check(o18 && bp.expanded === 'true' && bp.border === bp.accent, `S18：整頁重畫後新的鈴鐺仍為 aria-expanded="true" 且呈現按下狀態（實際 ${J(bp.expanded)} ${bp.border}）`);
    await cdp.press('Escape');
    await sleep(200);
    bp = await cdp.eval(bellProbeExpr);
    check(o18 && bp.expanded === 'false' && bp.border === closedBorder, `S18：按 Esc 關閉後 aria-expanded="false"、按下狀態消失（實際 ${J(bp.expanded)} ${bp.border}）`);
    const o18b = await openByMouse(cdp);
    await cdp.click('#app-name');
    await mouseAway();
    await sleep(300);
    bp = await cdp.eval(bellProbeExpr);
    check(o18b && !(await cdp.eval(panelOpenExpr)) && bp.expanded === 'false', `S18：點面板外關閉後 aria-expanded="false"（實際 ${J(bp.expanded)}）`);

    log('--- S19 鈴鐺不被頂列拉高（3.6 M2）---');
    for (const [w, h] of [[1536, 1024], [700, 900]]) {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
      await sleep(400);
      bp = await cdp.eval(bellProbeExpr);
      check(!!bp && bp.h <= 20 && Math.abs(bp.w - bp.h) <= 1 && Math.abs(bp.dy) <= 1.5, `S19：${w} 寬時鈴鐺約為正方形、不超過 20px 高且在頂列垂直置中（實際 ${bp && bp.w}×${bp && bp.h}、偏移 ${bp && bp.dy}）`);
    }
    await cdp.send('Emulation.clearDeviceMetricsOverride');
    await sleep(300);

    log('--- S20 Esc 已被其他浮層處理（defaultPrevented）時不關面板（3.6 M5）---');
    const o20 = await openByMouse(cdp);
    p = await cdp.eval(panelExpr);
    pre(o20 && p.focusIsFirstToggle, `S20 前提：面板開啟、焦點在第一個開關（實際 ${J(p.focusKind)}）`);
    await cdp.eval("(() => { const t = document.querySelector('#notify-panel [data-notify-kind]'); t.addEventListener('keydown', (e) => { if (e.key === 'Escape') e.preventDefault(); }, { once: true }); return true; })()");
    await cdp.press('Escape');
    await sleep(200);
    check(o20 && (await cdp.eval(panelOpenExpr)), 'S20：Esc 已被焦點所在元素的處理常式 preventDefault 時，面板不關閉');
    await cdp.press('Escape');
    await sleep(200);
    f = await cdp.eval(focusExpr);
    check(o20 && !(await cdp.eval(panelOpenExpr)) && f.onBell, `S20：下一次未被處理的 Esc 照常關閉、焦點回鈴鐺（實際 ${J(f)}）`);

    log('--- S21 面板的 Tab 順序接在鈴鐺之後（3.6 M9）---');
    pre(f.onBell, 'S21 前提：焦點在鈴鐺上');
    await cdp.press('Enter');
    await cdp.waitFor(panelOpenExpr, 2000);
    p = await cdp.eval(panelExpr);
    pre(p.open && p.focusIsFirstToggle, `S21 前提：以 Enter 開啟、焦點在第一個開關（實際 ${J(p.focusKind)}）`);
    await cdp.press('ShiftTab');
    await sleep(150);
    f = await cdp.eval(focusExpr);
    check(p.open && f.onBell && (await cdp.eval(panelOpenExpr)), `S21：在第一個開關按 Shift+Tab，焦點回到鈴鐺、面板仍開啟（實際 ${J(f)}）`);
    await cdp.press('Tab');
    await sleep(150);
    p = await cdp.eval(panelExpr);
    check(f.onBell && p.open && p.focusIsFirstToggle, `S21：面板開著時在鈴鐺上按 Tab，焦點進到面板的第一個開關（實際焦點類別 ${J(p.focusKind)}）`);
    const lastExpr = "(() => { const all = Array.from(document.querySelectorAll('#notify-panel input, #notify-panel button, #notify-panel [tabindex]')).filter((n) => !n.disabled && n.tabIndex >= 0 && n.getClientRects().length > 0); return all.length > 0 && document.activeElement === all[all.length - 1]; })()";
    let toLast = 0;
    for (let i = 0; i < 8 && !(await cdp.eval(lastExpr)); i++) {
      await cdp.press('Tab');
      toLast += 1;
    }
    const atLast = await cdp.eval(lastExpr);
    pre(atLast && (await cdp.eval(panelOpenExpr)), `S21 前提：Tab ${toLast} 次到面板最後一個可聚焦元素、面板仍開啟`);
    await cdp.press('Tab');
    await sleep(200);
    f = await cdp.eval(focusExpr);
    check(atLast && !(await cdp.eval(panelOpenExpr)) && f.onBell, `S21：在面板最後一個元素按 Tab，面板關閉、焦點回到鈴鐺（實際 ${J(f)}）`);

    log('--- S22 面板留在視窗內（3.6 M4；700×500、頁面已捲動）---');
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 700, height: 500, deviceScaleFactor: 1, mobile: false });
    try {
      await sleep(500);
      // S21 失敗時焦點可能不在鈴鐺、面板可能還開著：先收拾，讓 S22 的前提獨立成立。
      if (await cdp.eval(panelOpenExpr)) await cdp.eval('window.cockpitNotify.togglePanel(); true');
      f = await cdp.eval(focusExpr);
      if (!f.onBell) await tabUntil(cdp, `(a) => a.matches(${J(BELL)})`, 60);
      f = await cdp.eval(focusExpr);
      pre(f.onBell, `S22 前提：焦點在鈴鐺上（實際 ${J(f)}）`);
      await cdp.eval('window.scrollTo(0, 400); true');
      await sleep(300);
      let pv = await cdp.eval(panelViewportExpr);
      pre(pv.scrollY >= 300 && pv.bell && pv.bell.bottom <= 0, `S22 前提：頁面已往下捲、鈴鐺在視窗上方之外（實際 ${J(pv)}）`);
      await cdp.press('Enter');
      await cdp.waitFor(panelOpenExpr, 2000);
      await sleep(200);
      pv = await cdp.eval(panelViewportExpr);
      p = await cdp.eval(panelExpr);
      check(p.open && pv.panelIn && pv.toggleIn && p.focusIsFirstToggle, `S22：鈴鐺被捲走時以鍵盤開啟，面板與取得焦點的第一個開關都在視窗內（實際 ${J(pv)}）`);
      await cdp.eval('window.scrollTo(0, document.scrollingElement.scrollHeight); true');
      await sleep(300);
      pv = await cdp.eval(panelViewportExpr);
      check(p.open && (await cdp.eval(panelOpenExpr)) && pv.panelIn, `S22：面板開著時把頁面捲到底，面板仍在視窗內（實際 ${J(pv)}）`);
      await cdp.eval('window.scrollTo(0, 0); true');
      await sleep(300);
      pv = await cdp.eval(panelViewportExpr);
      check(p.open && pv.panelIn && !!pv.bell && pv.panel.top >= pv.bell.bottom, `S22：捲回頂端後面板回到鈴鐺下方（實際 ${J(pv)}）`);
      await cdp.press('Escape');
      await sleep(200);
    } finally {
      await cdp.send('Emulation.clearDeviceMetricsOverride');
      await cdp.eval('window.scrollTo(0, 0); true');
    }

    log('--- S23 寫入進行中按鈴鐺，稍後的失敗仍顯示（3.6 M1）---');
    pre(!(await cdp.eval("!!document.querySelector('#app .error-banner')")), 'S23 前提：目前沒有錯誤訊息');
    const tFail = Date.now();
    const clickedSlow = await cdp.click(`#app [data-action="fail"][data-project="cockpit"][data-task="docs-1"]`);
    pre(clickedSlow, `按 docs-1 的「Failed」（ui_preview 1500 ms 後回 409）`);
    const o23 = await openByMouse(cdp);
    if (o23) await cdp.click(BELL);
    const bellAt = Date.now() - tFail;
    const pendingAtBell = !(await cdp.eval("!!document.querySelector('#app .error-banner')"));
    pre(o23 && bellAt < 1200 && pendingAtBell, `寫入回應之前（${bellAt} ms）按鈴鐺開啟再關閉，當時還沒有錯誤訊息`);
    const shown = await cdp.waitFor("(() => { const b = document.querySelector('#app .error-banner'); return !!b && b.textContent.includes('/tasks/docs-1/fail'); })()", 4000);
    check(clickedSlow && o23 && shown, `S23 鈴鐺不改變進行中的操作狀態：寫入進行中按鈴鐺，該寫入稍後失敗的錯誤訊息仍出現（實際 ${J(await cdp.eval("(document.querySelector('#app .error-banner') || {}).textContent || null"))}）`);
    if (shown) await cdp.click('#app .error-banner [data-action="error-dismiss"]');

    log('--- P 段：直接呼叫純函式 ---');
    const results = await cdp.fn(pureChecks, P_LABELS);
    if (results.missing) {
      for (const label of P_LABELS) check(false, `${label}（window.cockpitNotify.diff／toNotifications 不存在）`);
    } else {
      for (const x of results.list) check(x.ok, `${x.label}${x.ok ? '' : `（實際 ${x.detail}）`}`);
    }
    const exAll = exceptionsSince(cdp, t0).filter((e) => !/模擬：本機儲存不可用/.test(e.text));
    pre(exAll.length === 0, `S 段沒有其他未捕捉例外（實際 ${J(exAll)}）`);
  } finally {
    await cdp.send('Page.navigate', { url: 'about:blank' });
    await sleep(300);
    killTree(srv.proc, 'ui_preview（S 段）');
  }
}

// ---------------------------------------------------------------------------
// T 段：同源的兩個分頁（ui_preview 四；5.3 審查 I2）
// ---------------------------------------------------------------------------

// 在同一個瀏覽器（同一個 user-data-dir＝共用 localStorage）開第二個分頁，掛上同一份注入腳本。
async function openSecondTab(cdpPort, url) {
  const r = await fetch(`http://127.0.0.1:${cdpPort}/json/new?${encodeURIComponent('about:blank')}`, { method: 'PUT' });
  const target = await r.json();
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = rej;
  });
  const cdp = new CDP(ws);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: `(${fakeInit.toString()})();` });
  await cdp.send('Page.navigate', { url });
  const ok = await cdp.waitFor(
    "document.readyState === 'complete' && document.querySelectorAll('#app .pane-row').length >= 1 && document.getElementById('version').textContent !== ''",
    10000
  );
  pre(ok, '第二個分頁載入並畫出第一份投影');
  await sleep(300);
  return { target, ws, cdp, ok };
}

// 以 observe 直接餵「wJ:p4 working → done」兩份狀態，回傳新增的通知標題（頁面目前生效的設定決定發不發）。
async function doneProbe() {
  const N = window.cockpitNotify;
  if (!N || typeof N.observe !== 'function') return { missing: true };
  const s = await (await fetch('/api/state')).json();
  const make = (status) => {
    const x = JSON.parse(JSON.stringify(s));
    for (const r of x.runtimes) for (const w of r.workspaces || []) for (const t of w.tabs || []) for (const p of t.panes || []) if (p.id === 'wJ:p4') p.agent_status = status;
    return x;
  };
  const before = window.__notifs.length;
  N.observe(make('working'));
  N.observe(make('done'));
  return { missing: false, recs: window.__notifs.slice(before).map((n) => n.title) };
}

async function partT(chrome, port) {
  log('=== T 段：同源兩個分頁 ===');
  const { cdp: A, cdpPort } = chrome;
  startServer.vanish = undefined;
  const srv = startServer(port, { transitions: ['3600000:pane:win/wJ:p4=idle'] });
  let second = null;
  try {
    if (!pre(await waitUp(port), `ui_preview（T 段）在 10 秒內開始回應；stderr：${srv.stderr().slice(0, 200)}`)) return;
    const url = `http://127.0.0.1:${port}/`;
    await reset(A, url, { permission: 'granted', focused: false, visibility: null, unsupported: false, storageThrows: false });
    second = await openSecondTab(cdpPort, url);
    if (!second.ok) return;
    const B = second.cdp;
    const t0 = Date.now();
    const DONE_ON = { blocked: true, done: true, failed: true, completed: false };

    log('--- T1 乙先開著面板，甲改 done ---');
    await B.send('Page.bringToFront');
    const openedB = await openByMouse(B);
    let pb = await B.eval(panelExpr);
    check(openedB && sameJson(pb.state, DEFAULTS), `T1：乙開啟面板，初始為預設值（實際 ${J(pb.state)}）`);
    let probe = await B.fn(doneProbe);
    check(!probe.missing && probe.recs.length === 0, `T1：乙目前設定下 done 關閉，working→done 不發通知（實際 ${J(probe)}）`);
    await A.send('Page.bringToFront');
    const openedA = await openByMouse(A);
    const clickedA = openedA && (await A.click('#notify-panel [data-notify-kind="done"]'));
    await sleep(300);
    let pa = await A.eval(panelExpr);
    check(clickedA && pa.state.done === true, `T1：甲以滑鼠開啟 done（實際 ${J(pa.state)}）`);
    let stored = await A.eval(`window.__lsGet(${J(STORAGE_KEY)})`);
    check(sameJson(safeParse(stored), DONE_ON), `T1：本機儲存為 done 開（實際 ${J(stored)}）`);

    log('--- T2 乙開著的面板與乙之後的通知跟著變 ---');
    const synced = await B.waitFor(`(${panelInfo.toString()})().state.done === true`, 2000);
    pb = await B.eval(panelExpr);
    check(synced && pb.open && sameJson(pb.state, DONE_ON), `T2：甲改 done 後，乙開著的面板立即顯示 done 已勾且仍開著（實際 open=${pb.open} ${J(pb.state)}）`);
    probe = await B.fn(doneProbe);
    check(!probe.missing && probe.recs.length === 1 && probe.recs[0] === 'agent 停下等你看', `T2：乙之後的 working→done 以新設定發出「agent 停下等你看」（實際 ${J(probe)}）`);

    log('--- T3 乙再改 failed，不蓋掉甲的 done ---');
    await B.send('Page.bringToFront');
    const clickedB = pb.open && (await B.click('#notify-panel [data-notify-kind="failed"]'));
    await sleep(300);
    pb = await B.eval(panelExpr);
    check(clickedB && pb.state.failed === false && pb.state.done === true, `T3：乙關閉 failed 後面板為 done 開、failed 關（實際 ${J(pb.state)}）`);
    stored = await B.eval(`window.__lsGet(${J(STORAGE_KEY)})`);
    check(sameJson(safeParse(stored), { blocked: true, done: true, failed: false, completed: false }), `T3：本機儲存同時保有甲的 done 開與乙的 failed 關（實際 ${J(stored)}）`);
    const syncedA = await A.waitFor(`(${panelInfo.toString()})().state.failed === false`, 2000);
    pa = await A.eval(panelExpr);
    check(syncedA && pa.open && pa.state.done === true && pa.state.failed === false, `T3：甲開著的面板跟著顯示 failed 關、done 仍開（實際 open=${pa.open} ${J(pa.state)}）`);
    probe = await A.fn(doneProbe);
    check(!probe.missing && probe.recs.length === 1, `T3：甲之後的 working→done 仍發通知（沒被乙的變更洗掉；實際 ${J(probe)}）`);

    const ex = [...A.exceptions, ...B.exceptions].filter((e) => e.at >= t0);
    pre(ex.length === 0, `T 段沒有未捕捉例外（實際 ${J(ex)}）`);
  } finally {
    if (second) {
      try {
        second.ws.close();
      } catch {
        // 已斷線。
      }
      await fetch(`http://127.0.0.1:${cdpPort}/json/close/${second.target.id}`).catch(() => {});
    }
    await A.send('Page.bringToFront').catch(() => {});
    await A.send('Page.navigate', { url: 'about:blank' });
    await sleep(300);
    killTree(srv.proc, 'ui_preview（T 段）');
  }
}

function safeParse(s) {
  try {
    return JSON.parse(s);
  } catch {
    return null;
  }
}

// P 段的斷言標籤（頁面內依序號引用；模組不存在時逐條記 FAIL，紅綠兩種跑法條數一致）。
const P_LABELS = [
  'P1：兩份相同的狀態不產生事件',
  'P2：wJ:p1 working→blocked 產生一則「agent 卡住」，內文綁定名稱去重（Backend 兩個 workstream 只列一次）',
  'P3：blocked 關閉時不產生通知',
  'P4：只算 binding.state=bound 的 workstream（ambiguous 候選不算）',
  'P5：沒有綁定的 pane 內文只有「runtime / pane」',
  'P6：新出現的 pane（即使 blocked）不產生事件',
  'P7：exited 的 pane 由 done 變 blocked 不產生事件',
  'P8：新出現的 task（即使 failed）不產生事件',
  'P9：r2 的 wJ:p1 變 blocked 只產生 r2 的事件，且不套用 runtime 不符的綁定',
  'P10：done 預設關閉不產生通知',
  'P11：done 開啟後標題「agent 停下等你看」',
  'P12：completed 預設關閉不產生通知',
  'P13：completed 開啟後「task completed」，內文「project 名稱：task 標題」',
  'P14：task running→failed「task failed」',
  'P15：failed→failed 不重複產生事件',
  'P16：同一份狀態 3 個事件時逐一發出 3 則',
  'P17：3 個 failed＋1 個 done（done 關閉）仍逐一 3 則（關閉的類別不產生事件、不計入件數）',
  'P18：5 個事件合併成一則「Cockpit：5 件事需要注意」，內文列 3 件、以「…」結尾，tag cockpit:summary',
  'P19：blocked→done 產生 done 事件（且不產生 blocked 事件）'
];

// P 段：在頁面內以 /api/state 為底做出前後兩份狀態，直接呼叫 diff 與 toNotifications。
async function pureChecks(labels) {
  const N = window.cockpitNotify;
  if (!N || typeof N.diff !== 'function' || typeof N.toNotifications !== 'function') return { missing: true };
  const S = await (await fetch('/api/state')).json();
  const clone = (x) => JSON.parse(JSON.stringify(x));
  const panes = (s, rt) => {
    const out = [];
    for (const r of s.runtimes) if (!rt || r.id === rt) for (const w of r.workspaces || []) for (const t of w.tabs || []) for (const p of t.panes || []) out.push(p);
    return out;
  };
  const pane = (s, rt, id) => panes(s, rt).find((p) => p.id === id);
  const task = (s, proj, id) => s.projects.find((p) => p.id === proj).tasks.find((t) => t.id === id);
  const D = { blocked: true, done: false, failed: true, completed: false };
  const ALL = { blocked: true, done: true, failed: true, completed: true };
  const list = [];
  const add = (n, ok, detail) => list.push({ label: labels[n - 1], ok: !!ok, detail: JSON.stringify(detail) });
  const run = (prev, next, settings) => {
    const ev = N.diff(prev, next);
    return { ev, notes: N.toNotifications(ev, settings) };
  };
  const pick = (n) => n.map((x) => ({ title: x.title, body: x.body, tag: x.tag }));
  // 基準：wJ:p1 working、p3 unknown、p4 idle、p2 done（exited）；be-1 running。
  const base = clone(S);
  pane(base, 'win', 'wJ:p1').agent_status = 'working';
  pane(base, 'win', 'wJ:p3').agent_status = 'unknown';
  pane(base, 'win', 'wJ:p4').agent_status = 'idle';
  task(base, 'cockpit', 'be-1').status = 'running';

  let r = run(base, clone(base), D);
  add(1, r.ev.length === 0 && r.notes.length === 0, r);

  let next = clone(base);
  pane(next, 'win', 'wJ:p1').agent_status = 'blocked';
  r = run(base, next, D);
  add(2,
    r.ev.length === 1 && JSON.stringify(pick(r.notes)) === JSON.stringify([{ title: 'agent 卡住', body: 'win / wJ:p1（Backend、Undeclared）', tag: 'cockpit:blocked:win/wJ:p1' }]),
    pick(r.notes)
  );
  r = run(base, next, { ...D, blocked: false });
  add(3, r.notes.length === 0, pick(r.notes));

  next = clone(base);
  pane(next, 'win', 'wJ:p3').agent_status = 'blocked';
  r = run(base, next, D);
  add(4, JSON.stringify(pick(r.notes)) === JSON.stringify([{ title: 'agent 卡住', body: 'win / wJ:p3（QA）', tag: 'cockpit:blocked:win/wJ:p3' }]), pick(r.notes));

  next = clone(base);
  pane(next, 'win', 'wJ:p4').agent_status = 'blocked';
  r = run(base, next, D);
  add(5, JSON.stringify(pick(r.notes)) === JSON.stringify([{ title: 'agent 卡住', body: 'win / wJ:p4', tag: 'cockpit:blocked:win/wJ:p4' }]), pick(r.notes));

  // 新出現的 pane（上一份沒有）不產生事件。
  next = clone(base);
  const tab = next.runtimes.find((x) => x.id === 'win').workspaces[0].tabs[0];
  tab.panes.push({ ...clone(pane(base, 'win', 'wJ:p4')), id: 'wJ:p9', agent_status: 'blocked' });
  r = run(base, next, ALL);
  add(6, r.ev.length === 0 && r.notes.length === 0, r);

  // exited 的 pane 不產生事件。
  next = clone(base);
  pane(next, 'win', 'wJ:p2').agent_status = 'blocked';
  r = run(base, next, ALL);
  add(7, r.ev.length === 0 && r.notes.length === 0, r);

  // 新出現的 task（failed）不產生事件。
  next = clone(base);
  next.projects[0].tasks.push({ ...clone(task(base, 'cockpit', 'be-1')), id: 'new-1', status: 'failed', mark: 'failed' });
  r = run(base, next, ALL);
  add(8, r.ev.length === 0 && r.notes.length === 0, r);

  // pane 以 runtime id＋pane id 識別：另一個 runtime 的同名 pane，綁定只認 runtime 相符者。
  const prev2 = clone(base);
  const r2 = clone(prev2.runtimes.find((x) => x.id === 'win'));
  r2.id = 'r2';
  prev2.runtimes.push(r2);
  next = clone(prev2);
  pane(next, 'r2', 'wJ:p1').agent_status = 'blocked';
  r = run(prev2, next, D);
  add(9, JSON.stringify(pick(r.notes)) === JSON.stringify([{ title: 'agent 卡住', body: 'r2 / wJ:p1', tag: 'cockpit:blocked:r2/wJ:p1' }]), pick(r.notes));

  // done：預設關、開啟後標題。
  next = clone(base);
  pane(next, 'win', 'wJ:p4').agent_status = 'done';
  r = run(base, next, D);
  add(10, r.notes.length === 0, pick(r.notes));
  r = run(base, next, { ...D, done: true });
  add(11, JSON.stringify(pick(r.notes)) === JSON.stringify([{ title: 'agent 停下等你看', body: 'win / wJ:p4', tag: 'cockpit:done:win/wJ:p4' }]), pick(r.notes));

  // task completed／failed。
  next = clone(base);
  task(next, 'cockpit', 'be-1').status = 'completed';
  r = run(base, next, D);
  add(12, r.notes.length === 0, pick(r.notes));
  r = run(base, next, { ...D, completed: true });
  add(13, JSON.stringify(pick(r.notes)) === JSON.stringify([{ title: 'task completed', body: 'AI Cockpit：投影擴充', tag: 'cockpit:completed:cockpit/be-1' }]), pick(r.notes));
  next = clone(base);
  task(next, 'cockpit', 'be-1').status = 'failed';
  r = run(base, next, D);
  add(14, JSON.stringify(pick(r.notes)) === JSON.stringify([{ title: 'task failed', body: 'AI Cockpit：投影擴充', tag: 'cockpit:failed:cockpit/be-1' }]), pick(r.notes));
  r = run(next, clone(next), D);
  add(15, r.ev.length === 0, r);

  // 合併的邊界：3 件逐一、4 件合併；關閉的類別不計入件數。
  const failTasks = (s, ids) => {
    const n = clone(s);
    for (const id of ids) task(n, 'cockpit', id).status = 'failed';
    return n;
  };
  next = failTasks(base, ['be-1', 'docs-1', 'release-1']);
  r = run(base, next, D);
  add(16, r.notes.length === 3 && r.notes.every((x) => x.tag !== 'cockpit:summary'), pick(r.notes));
  pane(next, 'win', 'wJ:p4').agent_status = 'done';
  r = run(base, next, D);
  add(17, r.notes.length === 3 && r.notes.every((x) => x.tag !== 'cockpit:summary'), pick(r.notes));
  next = failTasks(base, ['be-1', 'docs-1', 'release-1', 'be-2', 'qa-2']);
  r = run(base, next, D);
  const titles = ['投影擴充', 'README', '發布準備', '介面草稿', '回歸清單'];
  const s0 = r.notes[0] || {};
  const listed = titles.filter((t) => (s0.body || '').includes(t));
  add(18,
    r.notes.length === 1 && s0.title === 'Cockpit：5 件事需要注意' && listed.length === 3 && (s0.body || '').trimEnd().endsWith('…') && s0.tag === 'cockpit:summary',
    pick(r.notes)
  );
  // 由 blocked 變 done 也算「由其他值變成 done」。
  const prevB = clone(base);
  pane(prevB, 'win', 'wJ:p4').agent_status = 'blocked';
  next = clone(base);
  pane(next, 'win', 'wJ:p4').agent_status = 'done';
  r = run(prevB, next, ALL);
  add(19, JSON.stringify(pick(r.notes).map((x) => x.tag)) === JSON.stringify(['cockpit:done:win/wJ:p4']), pick(r.notes));
  return { missing: false, list };
}

// ---------------------------------------------------------------------------
// 截圖（desktop-launch-notify task 3.4；--screenshots）
// ---------------------------------------------------------------------------

// 截圖去識別化：作法同 docs/research/2026-10-02/output-color-check.js 的 maskExpression。ui_preview 的 fixture 把
// pane cwd 放在真實 %TEMP% 底下，畫面會出現 `C:\Users\<真實使用者名稱>\AppData\Local\Temp\…`。截圖前在頁面裝
// MutationObserver，把文字節點中 `Users\` 之後的路徑段與真實使用者名稱、主機名稱換成 `<user>`；背景重畫
// 一直重建節點，所以要持續替換。名稱在執行時由 os 取得，不寫進 repo。
function maskExpression() {
  const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const names = [os.userInfo().username, os.hostname()].filter(Boolean);
  const nameAlt = names.map(esc).join('|');
  return `(() => {
    const userRe = /(Users[\\\\/])[^\\\\/\\s]+/gi;
    const nameRe = new RegExp(${JSON.stringify(nameAlt)}, 'gi');
    const mask = (s) => s.replace(userRe, '$1<user>').replace(nameRe, '<user>');
    const sweep = (root) => {
      const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      for (let n = w.nextNode(); n; n = w.nextNode()) {
        const v = mask(n.nodeValue);
        if (v !== n.nodeValue) n.nodeValue = v;
      }
    };
    window.__maskObserver && window.__maskObserver.disconnect();
    window.__maskObserver = new MutationObserver(() => sweep(document.body));
    window.__maskObserver.observe(document.body, { childList: true, subtree: true, characterData: true });
    sweep(document.body);
    return true;
  })()`;
}

// 截圖前確認頁面文字（textContent，含被 CSS 截斷的部分）不含真實使用者名稱或主機名稱。
async function assertMasked(cdp, width) {
  const names = [os.userInfo().username, os.hostname()].filter(Boolean);
  const text = await cdp.eval('document.body.textContent');
  const hit = names.filter((n) => text.toLowerCase().includes(n.toLowerCase()));
  pre(hit.length === 0, `截圖 ${width}：截圖前頁面文字不含真實使用者名稱與主機名稱（命中 ${hit.length} 個）`);
  return hit.length === 0;
}

// 設定面板開啟時，1536×1024 與 700×900 各截一張。權限選「尚未決定」（default）：面板同時顯示權限說明、
// 「允許通知」按鈕與四個預設開關（blocked、failed 開；done、completed 關），是各狀態中資訊最完整的一種。
// 面板一律以真實滑鼠點鈴鐺開啟；每個寬度都重新開啟，確認該寬度下面板與鈴鐺可操作。
async function screenshotCase(cdp, port) {
  log('=== 截圖：設定面板（1536／700）===');
  // 一小時後才套用的無害轉換，只為停掉 wJ:p1 的輪替，讓兩張圖的畫面一致。
  const srv = startServer(port, { transitions: ['3600000:pane:win/wJ:p4=idle'] });
  try {
    if (!pre(await waitUp(port), `ui_preview（截圖）在 10 秒內開始回應；stderr：${srv.stderr().slice(0, 200)}`)) return;
    const url = `http://127.0.0.1:${port}/`;
    await reset(cdp, url, { permission: 'default', requestResult: 'granted', focused: false, visibility: null, unsupported: false, storageThrows: false });
    await cdp.eval(maskExpression());
    for (const [w, h] of [[1536, 1024], [700, 900]]) {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
      await sleep(500);
      if (await cdp.eval(panelOpenExpr)) {
        await cdp.press('Escape');
        await sleep(300);
      }
      const opened = await openByMouse(cdp);
      pre(opened, `截圖 ${w}：以滑鼠按鈴鐺開啟設定面板`);
      if (!opened) continue;
      await sleep(500);
      const p = await cdp.eval(panelExpr);
      pre(p.buttons.includes('允許通知') && sameJson(p.state, DEFAULTS), `截圖 ${w}：面板顯示「允許通知」且四個開關為預設值（按鈕 ${J(p.buttons)}、開關 ${J(p.state)}）`);
      if (!(await assertMasked(cdp, w))) continue;
      const shot = await cdp.send('Page.captureScreenshot', { format: 'png' });
      const file = path.join(__dirname, `notify-panel-${w}.png`);
      fs.writeFileSync(file, Buffer.from(shot.result.data, 'base64'));
      log(`wrote ${file}`);
    }
    await cdp.send('Emulation.clearDeviceMetricsOverride');
  } finally {
    await cdp.send('Page.navigate', { url: 'about:blank' });
    await sleep(300);
    killTree(srv.proc, 'ui_preview（截圖）');
  }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

async function main() {
  if (!fs.existsSync(CHROME)) throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  if (!fs.existsSync(UI_PREVIEW_EXE)) throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  const port = pickPort(7770);
  if (port !== 7770) log(`7770 已被占用，改用埠 ${port}`);
  const cdpPort = pickPort(18810, [port]);
  let chrome = null;
  try {
    chrome = await startChrome(cdpPort);
    const { cdp } = chrome;
    const only = process.argv.slice(2).find((a) => /^--only=/.test(a));
    const parts = only ? only.slice(7).split(',') : ['N', 'M', 'S', 'T'];
    if (parts.includes('N')) await partN(cdp, port);
    if (parts.includes('M')) await partM(cdp, port);
    if (parts.includes('S')) await partS(cdp, port);
    if (parts.includes('T')) await partT(chrome, port);
    if (process.argv.includes('--screenshots')) await screenshotCase(cdp, port);
  } catch (e) {
    pre(false, `中止：${e.stack || e.message}`);
  } finally {
    await stopChrome(chrome);
    await sleep(300);
    pre(!isPortListening(port), `port ${port} 應該不再有 LISTENING 的行程`);
  }
  console.log(`新行為斷言：${tally.spec.pass + tally.spec.fail} 條，PASS ${tally.spec.pass}、FAIL ${tally.spec.fail}`);
  console.log(`前置斷言：${tally.pre.pass + tally.pre.fail} 條，PASS ${tally.pre.pass}、FAIL ${tally.pre.fail}`);
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
