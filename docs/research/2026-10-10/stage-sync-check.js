// stage-sync-check.js：openspec-stage-sync 前端驗收腳本（openspec-stage-sync task 5.1 建立；5.2 加入「編輯 stage」
// 對話框階段下拉與「加入」phases 的段落；5.3 設計審核修正加入對話框說明文字、下拉外觀與超長 change 名稱的段落；
// 之後的 task 往上加）。
// headless Chrome＋raw CDP。啟動、行程所有權模型、段落代號與輸出格式沿用
// docs/research/2026-10-08/repo-projects-check.js（startPreview／startChrome／killTree、parseSegmentArg、
// 打錯段名 exit 2、以 #version 的 data-state-version 判斷整頁重畫）。
//
// 對 `cockpit --example ui_preview` 驗 openspec/changes/openspec-stage-sync/specs/cockpit-dashboard/spec.md
// 「卡片的 OpenSpec 同步標示」與 specs/ui-language/spec.md「OpenSpec 同步相關介面文字」中屬於卡片標示的 scenario。
// fixture：demo-app 兩張卡片——add-login（implement 3/8、auto，在 Build）與 fix-cache（review 5/5、manual，在 Plan）；
// 其餘 Project 的 task 沒有 sync（null）。5.2 的段落（dialog/、add/）驗 specs/cockpit-dashboard/spec.md「Project 切換」的
// 「編輯 stage」階段下拉與「加入」本體：demo-app 的 stages 為 Plan、Build，stage_phases 為 [plan, implement]；送出的請求
// 本體取自 ui_preview stdout 的 `write-request <METHOD> <PATH> <BODY>` 記錄行（本體逐字，POST／PATCH 的本體不改投影）。
//
// 用法（repo 根；先 `cargo build -p cockpit --example ui_preview`——前端資產內嵌在執行檔裡，改了
// cockpit/assets/ 沒重建就是驗舊版）：
//   node docs/research/2026-10-10/stage-sync-check.js                 # 全部段落
//   node docs/research/2026-10-10/stage-sync-check.js "card/"         # 只跑指定段落（逗號分隔；`<前綴>/` 選該前綴全部）
// 段落代號拼錯、空字串或只有逗號 → 印 `RESULT: FAIL (段落代號)`、exit 2，不啟動任何行程。
//
// 埠：preview 從 7930 起、CDP 從 19710 起（pickPort 遇到占用就往上找）。2026-10-10 grep docs/research/*/*.js 的
// pickPort／PORT 常數：preview 用過 7770、7780、7790、7792、7793、7800、7830、7870、7910、7950、7970、7990；
// CDP 用過 9333、18781–18991、19000–19200、19310、19410–19440、19510、19610，7930 與 19710 一帶沒有人用。
// 開跑前 7930 或 19710 已有人 LISTEN 就直接結束（exit 2），不搶、不動別人的行程。7770 有人 LISTEN 時只印一行「注意」
// （那是使用者的 cockpit）；7778 是與本專案無關的 ASUS 服務，本腳本不碰。
//
// 收尾殘留判定（ledger 裁決「調查（Task 4.6 carry）」）：Windows 會重用 PID，只比 PID 會把「晚幾秒建立的別的程式」
// 誤判成本腳本的殘留。所以 spawn 當下記下「PID＋建立時間」，收尾只有 PID 相同且建立時間也相同才算殘留。
'use strict';

const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

const PREVIEW_PORT_BASE = 7930;
const CDP_PORT_BASE = 19710;
const STABLE_PUSH_MS = '600000'; // 推送間隔拉長，讓背景輪替在段落期間靜止；需要頻繁重畫的段落以 env 覆寫。
const CHROME_UDD_PREFIX = 'cockpit-chrome-stagesynccheck-';
const PREVIEW_TEMP_PREFIX = 'cockpit-ui-preview-';
const UI_TIMEOUT_MS = 5000;

// 範例資料（cockpit/examples/ui_preview.rs 的 demo-app 兩張 task）。
const AUTO_CARD = { change: 'add-login', progress: '3/8' };
const MANUAL_CARD = { change: 'fix-cache', progress: '5/5' };
const MODE_TEXT = { zh: { auto: '自動', manual: '手動' }, en: { auto: 'Auto', manual: 'Manual' } };
const MIN_CONTRAST = 4.5;

// ---------------------------------------------------------------------------
// 命令列
// ---------------------------------------------------------------------------

const POSITIONAL = process.argv.slice(2);
const SEGMENT_ARG = POSITIONAL[0];

function parseSegmentArg(arg, knownCodes) {
  if (arg === undefined) return { ok: true, codes: null };
  const items = String(arg)
    .split(',')
    .map((s) => s.trim())
    .filter((s) => s !== '');
  if (items.length === 0) {
    return { ok: false, message: `沒有選中任何段落（參數 ${JSON.stringify(arg)}）；可用代號：${knownCodes.join(',')}` };
  }
  const codes = [];
  const unknown = [];
  for (const item of items) {
    if (item.endsWith('/')) {
      const hit = knownCodes.filter((c) => c.startsWith(item));
      if (hit.length === 0) unknown.push(item);
      for (const c of hit) if (!codes.includes(c)) codes.push(c);
    } else if (knownCodes.includes(item)) {
      if (!codes.includes(item)) codes.push(item);
    } else {
      unknown.push(item);
    }
  }
  if (unknown.length > 0) {
    return { ok: false, message: `未知的段落代號：${unknown.join(',')}；可用代號：${knownCodes.join(',')}` };
  }
  return { ok: true, codes };
}

// ---------------------------------------------------------------------------
// 斷言與輸出
// ---------------------------------------------------------------------------

const failures = []; // { segment, label }
let CURRENT_SEGMENT = null;
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push({ segment: CURRENT_SEGMENT, label });
  return !!cond;
}
// 前置條件：不成立就 FAIL 並結束這一段。
class SegmentAbort extends Error {}
function need(cond, label) {
  if (!check(cond, label)) throw new SegmentAbort(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const J = (v) => JSON.stringify(v);

// ---------------------------------------------------------------------------
// 行程管理：只終止本腳本自己 spawn、還握著 ChildProcess 且沒觀察到 exit 的行程；
// 殘留判定一律用「PID＋建立時間」。
// ---------------------------------------------------------------------------

// PID 的建立時間（ISO 8601，100ns 精度）；該 PID 不存在回傳 null。
function processCreationTime(pid) {
  const r = spawnSync(
    'powershell',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      `Get-CimInstance Win32_Process -Filter "ProcessId=${Number(pid)}" | ForEach-Object { $_.CreationDate.ToString('o') }`,
    ],
    { encoding: 'utf8' }
  );
  const t = (r.stdout || '').trim();
  return /^\d{4}-\d\d-\d\dT/.test(t) ? t : null;
}
// 這個身分（PID＋建立時間）的行程還活著嗎？同 PID 但建立時間不同＝PID 被別的程式重用，不算。
function identityAlive(identity) {
  const now = processCreationTime(identity.pid);
  return now !== null && now === identity.created;
}
// spawn 後立刻記下身分；行程剛起來偶爾查不到，重試幾次。
function recordIdentity(child) {
  if (!child.pid) return;
  let created = null;
  for (let i = 0; i < 5 && created === null; i += 1) created = processCreationTime(child.pid);
  OUR_IDENTITIES.push({ pid: child.pid, created });
}
// 只送終止；「是否真的結束了」交給 settleChild 輪詢判定（task 5.1 審查發現：剛 taskkill 就判定，行程還在結束中會誤報）。
function killTree(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
}
function isPortListening(port) {
  const r = spawnSync('netstat', ['-ano'], { encoding: 'utf8' });
  const needle = `127.0.0.1:${port} `;
  return (r.stdout || '').split('\n').some((line) => line.includes(needle) && line.includes('LISTENING'));
}
function pickPort(start, avoid = []) {
  let port = start;
  while (isPortListening(port) || avoid.includes(port)) port += 1;
  return port;
}
function runningUiPreviewPids() {
  const r = spawnSync('tasklist', ['/FI', 'IMAGENAME eq ui_preview.exe', '/NH', '/FO', 'CSV'], { encoding: 'utf8' });
  return (r.stdout || '')
    .split('\n')
    .map((l) => /^"ui_preview\.exe","(\d+)"/i.exec(l.trim()))
    .filter(Boolean)
    .map((m) => Number(m[1]));
}
// 本腳本的 headless Chrome（以 user-data-dir 前綴辨識）。
function ourChromePids() {
  const r = spawnSync(
    'powershell',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      "Get-CimInstance Win32_Process -Filter \"Name='chrome.exe'\" | " +
        `Where-Object { $_.CommandLine -like '*${CHROME_UDD_PREFIX}*' } | ForEach-Object { $_.ProcessId }`,
    ],
    { encoding: 'utf8' }
  );
  return (r.stdout || '')
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => /^\d+$/.test(s))
    .map(Number);
}

const SPAWNED = []; // { child, label, port }：尚未確認收乾淨的子行程
const OUR_PORTS = new Set();
const OUR_IDENTITIES = []; // { pid, created }：本次 spawn 過的行程身分（created 為 null 表示 spawn 當下查不到）
const PREVIEW_TEMP_ROOTS = new Set();
function hasObservedExit(child) {
  return child.exitCode !== null || child.signalCode !== null;
}
function waitForChildExit(child, timeoutMs) {
  if (hasObservedExit(child)) return Promise.resolve(true);
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      child.removeListener('exit', onExit);
      resolve(hasObservedExit(child));
    }, timeoutMs);
    function onExit() {
      clearTimeout(timer);
      resolve(true);
    }
    child.once('exit', onExit);
  });
}
// 輪詢到這個身分（PID＋建立時間）的行程不存在為止（taskkill 之後行程可能還要幾百毫秒才真的結束）。
async function waitIdentityGone(identity, timeoutMs) {
  const start = Date.now();
  for (;;) {
    if (!identityAlive(identity)) return true;
    if (Date.now() - start >= timeoutMs) return false;
    await sleep(200);
  }
}
async function settleChild(child, port, label) {
  const exited = await waitForChildExit(child, 5000);
  const portListening = isPortListening(port);
  if (exited && !portListening) {
    const i = SPAWNED.findIndex((e) => e.child === child);
    if (i !== -1) SPAWNED.splice(i, 1);
  }
  check(exited, `${label} 應該觀察到子行程的 exit 事件（PID ${child.pid}）`);
  check(!portListening, `${label} 的 port ${port} 應該不再有 LISTENING 的行程`);
  const identity = OUR_IDENTITIES.find((e) => e.pid === child.pid);
  if (identity && identity.created !== null) {
    check(await waitIdentityGone(identity, 5000), `${label} PID ${child.pid} 已終止（同 PID 且建立時間相同的行程已不存在，輪詢至多 5 秒）`);
  } else {
    // 查不到建立時間就沒有身分可比對：不掛一條永遠為真的 ok，只印注意，改靠上面的 exit 事件與 port 判定。
    log(`注意：${label} PID ${child.pid} 在 spawn 當下查不到建立時間，無法以身分確認已終止，僅靠 exit 事件與 port 判定`);
  }
}
function finalSweep() {
  let killed = 0;
  let residual = 0;
  for (const { child, label, port } of SPAWNED.slice()) {
    if (hasObservedExit(child)) {
      if (isPortListening(port)) {
        residual += 1;
        log(`最終清查警告：${label} 已 exit 但 port ${port} 仍 LISTENING（不終止任何 PID，需人工排查）`);
      }
      continue;
    }
    killed += 1;
    log(`最終清查：${label}（PID ${child.pid}）尚未觀察到 exit，補送終止`);
    spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
  }
  check(killed === 0, `最終清查：不應該有段落收尾漏掉、需要補送終止的行程（實際 ${killed} 個）`);
  check(residual === 0, `最終清查：不應該有「已 exit 但 port 仍 LISTENING」的殘留跡象（實際 ${residual} 個）`);
}

// ---------------------------------------------------------------------------
// CDP
// ---------------------------------------------------------------------------

class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.handlers = [];
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        this.pending.get(m.id)(m);
        this.pending.delete(m.id);
        return;
      }
      if (m.method) {
        for (const { method, handler } of this.handlers) {
          if (method === m.method) {
            try {
              handler(m.params, m.sessionId || null);
            } catch (err) {
              log(`CDP 事件處理例外（${m.method}）：${err.message}`);
            }
          }
        }
      }
    };
  }
  send(method, params = {}, sessionId = null) {
    const id = ++this.id;
    return new Promise((res) => {
      this.pending.set(id, res);
      const msg = { id, method, params };
      if (sessionId) msg.sessionId = sessionId;
      this.ws.send(JSON.stringify(msg));
    });
  }
  onEvent(method, handler) {
    this.handlers.push({ method, handler });
  }
  async eval(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.error) throw new Error(`Runtime.evaluate 失敗：${JSON.stringify(r.error)}`);
    if (r.result && r.result.exceptionDetails) {
      const d = r.result.exceptionDetails;
      throw new Error(`頁面內例外：${d.exception && d.exception.description ? d.exception.description : JSON.stringify(d)}`);
    }
    return r.result && r.result.result ? r.result.result.value : undefined;
  }
  run(fn, ...args) {
    return this.eval(`(${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')})`);
  }
  async poll(fn, args, timeoutMs, intervalMs = 100) {
    const start = Date.now();
    for (;;) {
      let v;
      try {
        v = await this.run(fn, ...args);
      } catch (e) {
        v = false;
      }
      if (v) return v;
      if (Date.now() - start >= timeoutMs) return false;
      await sleep(intervalMs);
    }
  }
  // 點擊 fn(...args) 回傳的元素：捲進可視範圍、確認點得到它，再送真的滑鼠事件。
  async clickEl(fn, args, desc) {
    const call = `(${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')})`;
    const locate = `(() => { const el = ${call};
      if (!el) return null; el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, w: r.width, h: r.height }; })()`;
    let pt = await this.eval(locate);
    if (!pt || pt.w === 0 || pt.h === 0) {
      check(false, `找不到可點的元素（或尺寸為 0）：${desc}`);
      return false;
    }
    for (let attempt = 1; ; attempt += 1) {
      await sleep(80);
      const hit = await this.eval(`(() => { const el = ${call};
        const h = document.elementFromPoint(${pt.x}, ${pt.y}); return !!el && !!h && (el === h || el.contains(h)); })()`);
      if (hit) break;
      if (attempt >= 5) {
        check(false, `座標上的元素不是目標本身或其子孫（重試 5 次）：${desc}`);
        return false;
      }
      pt = await this.eval(locate);
      if (!pt) {
        check(false, `元素在點擊前消失：${desc}`);
        return false;
      }
    }
    const base = { x: pt.x, y: pt.y, button: 'left', clickCount: 1 };
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: pt.x, y: pt.y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
    return true;
  }
  // 真的鍵盤事件（頁面 keydown 監聽器看得到 event.key）。ArrowDown：pressKey('ArrowDown', 'ArrowDown', 40, '')。
  async pressKey(key, code, windowsVirtualKeyCode, text, modifiers = 0) {
    await this.send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, windowsVirtualKeyCode, text, modifiers });
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode, modifiers });
  }
}

async function startChrome(cdpPort, url, label, windowSize = '1536,1024') {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), `${CHROME_UDD_PREFIX}${label.replace(/[^A-Za-z0-9-]/g, '')}-`));
  const chrome = spawn(
    CHROME,
    [
      '--headless=new',
      '--lang=zh-TW',
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
  chrome.on('error', (e) => check(false, `${label} chrome spawn error：${e.message}`));
  SPAWNED.push({ child: chrome, label: `${label}（chrome）`, port: cdpPort });
  OUR_PORTS.add(cdpPort);
  recordIdentity(chrome);
  const handle = { chrome, udd, cdpPort, ws: null, cdp: null };
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
  if (!page) {
    await stopChrome(handle, `${label}（啟動失敗收尾）`);
    throw new Error(`${label}: page target not found`);
  }
  try {
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((res, rej) => {
      ws.onopen = res;
      ws.onerror = rej;
    });
    handle.ws = ws;
    handle.cdp = new CDP(ws);
  } catch (e) {
    await stopChrome(handle, `${label}（啟動失敗收尾）`);
    throw new Error(`${label}: WebSocket 連線失敗：${e && e.message ? e.message : e}`);
  }
  return handle;
}

async function stopChrome(handle, label) {
  if (!handle) return;
  try {
    if (handle.ws) handle.ws.close();
  } catch {
    // 已斷線。
  }
  killTree(handle.chrome);
  await settleChild(handle.chrome, handle.cdpPort, label);
  await removeDirWithRetry(handle.udd);
}

async function removeDirWithRetry(dir) {
  for (let i = 0; i < 20; i++) {
    try {
      fs.rmSync(dir, { recursive: true, force: true });
    } catch {
      // Windows 偶爾 EBUSY／EPERM，稍後重試。
    }
    if (!fs.existsSync(dir)) return true;
    await sleep(250);
  }
  return !fs.existsSync(dir);
}

// ui_preview 啟動時把 file-review 的假 repo 複製到 %TEMP% 下（stdout 印 review-repo／other-repo 路徑），
// 以 taskkill /F 結束時沒有機會自己清，由 stopPreview() 刪。
async function startPreview(envOverrides, label) {
  if (!fs.existsSync(UI_PREVIEW_EXE)) {
    throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  }
  const port = pickPort(PREVIEW_PORT_BASE);
  const info = { reviewRepo: null, otherRepo: null };
  const writes = []; // { method, path, body }：ui_preview 的寫入端點只記錄不改投影（stdout 一行 write-request）
  const server = spawn(UI_PREVIEW_EXE, [], {
    stdio: ['ignore', 'pipe', 'ignore'],
    windowsHide: true,
    env: { ...process.env, COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`, COCKPIT_PREVIEW_PUSH_MS: STABLE_PUSH_MS, ...envOverrides },
  });
  server.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
  SPAWNED.push({ child: server, label: `${label}（preview）`, port });
  OUR_PORTS.add(port);
  recordIdentity(server);
  let buffer = '';
  server.stdout.setEncoding('utf8');
  server.stdout.on('data', (chunk) => {
    buffer += chunk;
    let nl;
    while ((nl = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, nl).replace(/\r$/, '');
      buffer = buffer.slice(nl + 1);
      let m = /^review-repo: (.+)$/.exec(line);
      if (m) info.reviewRepo = m[1].trim();
      m = /^other-repo: (.+)$/.exec(line);
      if (m) info.otherRepo = m[1].trim();
      m = /^write-request (\S+) (\S+) ?(.*)$/.exec(line);
      if (m) writes.push({ method: m[1], path: m[2], body: m[3] });
    }
  });
  let up = false;
  for (let i = 0; i < 50 && !(up && info.reviewRepo && info.otherRepo); i++) {
    if (!up) {
      try {
        up = (await fetch(`http://127.0.0.1:${port}/api/state`)).ok;
      } catch {
        // 還沒起來。
      }
    }
    if (!(up && info.reviewRepo && info.otherRepo)) await sleep(200);
  }
  const preview = { server, port, writes, ...info, tempRoot: info.reviewRepo ? path.dirname(info.reviewRepo) : null };
  if (preview.tempRoot) PREVIEW_TEMP_ROOTS.add(preview.tempRoot);
  if (!(up && info.reviewRepo && info.otherRepo)) {
    check(false, `${label} 應該在 10 秒內開始回應並印出 review-repo／other-repo 路徑（up=${up}）`);
    await stopPreview(preview, `${label}（啟動失敗收尾）`);
    throw new Error(`${label} 沒有起來`);
  }
  return preview;
}

async function stopPreview(preview, label) {
  if (!preview) return;
  killTree(preview.server);
  await settleChild(preview.server, preview.port, label);
  const root = preview.tempRoot;
  if (root && path.basename(root).startsWith(PREVIEW_TEMP_PREFIX)) {
    const removed = await removeDirWithRetry(root);
    check(removed, `${label} 的暫存副本目錄已刪除（${root}）`);
    if (removed) PREVIEW_TEMP_ROOTS.delete(root);
  }
}

// ---------------------------------------------------------------------------
// 頁面端工具：以 Page.addScriptToEvaluateOnNewDocument 安裝成 window.__ss（重新整理後仍在）。
// 定位規則（前端契約，stage-sync-check.md「前端契約」）：
//   S1 Factory Floor 的 task 節點＝[data-region="floor"] .task-node。
//   S2 同步標示＝節點的直接子元素 .task-sync（沒有 sync 就不存在）；內含 .task-sync-change（change 名稱）、
//      .task-sync-progress（checked/total）、.task-sync-mode（「自動」「手動」／Auto、Manual）；
//      .task-sync 的 data-sync-mode 為 auto 或 manual。
//   S3 「編輯 stage」對話框＝body 底下 <dialog class="project-dialog" data-dialog="stages">；每個 .stage-row 有
//      <select class="stage-phase">，option 的 value 依序為 ""（不對應）、plan、implement、review、complete。
// ---------------------------------------------------------------------------

function pageHelpers() {
  if (window.__ss) return;
  const txt = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : '');
  const stateVersion = () => {
    const v = document.getElementById('version');
    return v && v.hasAttribute('data-state-version') ? v.getAttribute('data-state-version') : null;
  };
  const projectItem = (id) =>
    Array.from(document.querySelectorAll('[data-region="projects"] [data-action="select-project"]')).find(
      (b) => b.getAttribute('data-project') === id
    ) || null;
  const shownProject = () => {
    const p = document.querySelector('[data-region="floor"] .project[data-project]');
    return p ? p.getAttribute('data-project') : null;
  };
  // 任何 CSS 顏色字串（含 color-mix 的計算結果）→ [r, g, b, a]（經 canvas 轉成 sRGB 8 位元）。
  const rgba = (color) => {
    const c = document.createElement('canvas');
    c.width = 1;
    c.height = 1;
    const g = c.getContext('2d', { willReadFrequently: true });
    g.clearRect(0, 0, 1, 1);
    g.fillStyle = color;
    g.fillRect(0, 0, 1, 1);
    return Array.from(g.getImageData(0, 0, 1, 1).data);
  };
  // 自己與祖先（到 body）的 opacity 乘積，用來確認「較淡」不是靠透明度做的（透明度會讓對比算不準）。
  const effectiveOpacity = (el) => {
    let o = 1;
    for (let n = el; n && n !== document.documentElement; n = n.parentElement) o *= Number(getComputedStyle(n).opacity);
    return o;
  };
  const nodes = () => Array.from(document.querySelectorAll('[data-region="floor"] .task-node'));
  const cards = () =>
    nodes().map((node) => {
      const sync = Array.from(node.children).find((c) => c.classList.contains('task-sync')) || null;
      const part = (cls) => (sync ? sync.querySelector(`.${cls}`) : null);
      const colorOf = (el) => (el ? rgba(getComputedStyle(el).color) : null);
      return {
        title: txt(node.querySelector('.task-title')),
        childClasses: Array.from(node.children).map((c) => c.className),
        anySyncInside: !!node.querySelector('.task-sync'),
        hasSync: !!sync,
        syncMode: sync ? sync.getAttribute('data-sync-mode') : null,
        syncText: txt(sync),
        syncTitle: sync ? sync.title : null,
        change: txt(part('task-sync-change')),
        progress: txt(part('task-sync-progress')),
        mode: txt(part('task-sync-mode')),
        imgInside: !!node.querySelector('img'),
        colors: { change: colorOf(part('task-sync-change')), progress: colorOf(part('task-sync-progress')), mode: colorOf(part('task-sync-mode')) },
        bg: rgba(getComputedStyle(node).backgroundColor),
        opacity: sync ? effectiveOpacity(sync) : null,
        fontSize: sync ? getComputedStyle(sync).fontSize : null,
        // 5.3：change 名稱的實際高度與單行行高（算行數）、是否被截斷（內容高於可視高）。
        changeBox: (() => {
          const ch = part('task-sync-change');
          if (!ch) return null;
          const cs = getComputedStyle(ch);
          return { h: ch.getBoundingClientRect().height, lh: parseFloat(cs.lineHeight), clipped: ch.scrollHeight > ch.clientHeight + 1, overflow: cs.overflow };
        })(),
      };
    });
  // 注入特製投影（同 visual-check.js injectState）：擋掉之後真的 /ws 推送，以原本的 onState 畫出 state。
  const inject = (state) => {
    if (!window.__ssOrigOnState) window.__ssOrigOnState = window.onState;
    window.onState = function () {};
    window.__ssOrigOnState(state);
    return true;
  };
  const latest = () => (typeof window.cockpitLatestState === 'function' ? JSON.parse(JSON.stringify(window.cockpitLatestState())) : null);
  // 「編輯 stage」對話框（body 底下、#app 之外的 <dialog class="project-dialog">；前端契約 S3）。
  const dialog = () => document.querySelector('dialog.project-dialog');
  const stageRows = () => (dialog() ? Array.from(dialog().querySelectorAll('.stage-row')) : []);
  const phaseSelect = (i) => (stageRows()[i] ? stageRows()[i].querySelector('select.stage-phase') : null);
  const dialogButton = (op, row) => {
    const d = dialog();
    if (!d) return null;
    if (row === null || row === undefined) return d.querySelector(`[data-dialog-op="${op}"]`) || d.querySelector(`[data-stage-op="${op}"]`);
    const r = stageRows()[row];
    return r ? r.querySelector(`[data-stage-op="${op}"]`) : null;
  };
  const describe = (a) => {
    if (!a || a === document.body) return null;
    const rows = stageRows();
    return { tag: a.tagName, cls: a.className || '', row: rows.findIndex((r) => r.contains(a)), op: a.getAttribute('data-stage-op') || a.getAttribute('data-dialog-op') };
  };
  const dialogSnap = () => {
    const d = dialog();
    if (!d) return null;
    const err = d.querySelector('.project-dialog-error');
    const selects = stageRows().map((r) => r.querySelector('select.stage-phase'));
    return {
      open: d.open,
      kind: d.open ? d.getAttribute('data-dialog') : null,
      inApp: !!document.getElementById('app') && document.getElementById('app').contains(d),
      stages: stageRows().map((r) => (r.querySelector('input.stage-name') || {}).value),
      phases: selects.map((x) => (x ? x.value : null)),
      shown: selects.map((x) => (x && x.selectedOptions[0] ? txt(x.selectedOptions[0]) : null)),
      options: selects.map((x) => (x ? Array.from(x.options).map((o) => [o.value, txt(o)]) : null)),
      labels: selects.map((x) => (x ? x.getAttribute('aria-label') : null)),
      tags: selects.map((x) => (x ? x.tagName : null)),
      error: err && !err.hidden ? txt(err) || null : null,
      active: describe(document.activeElement),
      dialogText: txt(d),
      hint: txt(d.querySelector('.project-dialog-text')),
      // 5.3：下拉顯示文字的計算顏色與自身背景（對比用）、名稱輸入框的背景。
      selectColors: selects.map((x) => (x ? { fg: rgba(getComputedStyle(x).color), bg: rgba(getComputedStyle(x).backgroundColor) } : null)),
      // 版面：每列的名稱輸入框、階段下拉、按鈕群的矩形；對話框是否超出視窗或橫向溢位。
      rects: stageRows().map((r) => {
        const rc = (el) => {
          if (!el) return null;
          const b = el.getBoundingClientRect();
          return { l: b.left, r: b.right, t: b.top, b: b.bottom, w: b.width, h: b.height };
        };
        return {
          row: rc(r),
          input: rc(r.querySelector('input.stage-name')),
          select: rc(r.querySelector('select.stage-phase')),
          actions: rc(r.querySelector('.stage-row-actions')),
        };
      }),
      dialogRect: (() => {
        const b = d.getBoundingClientRect();
        return { l: b.left, r: b.right, w: b.width, overflowX: d.scrollWidth > d.clientWidth };
      })(),
      viewportW: window.innerWidth,
    };
  };
  const addButton = (repo) =>
    Array.from(document.querySelectorAll('[data-region="projects"] [data-action="add-repo"]')).find((b) => b.getAttribute('data-repo') === repo) || null;
  const menuButton = (pid) =>
    Array.from(document.querySelectorAll('#app [data-action="project-menu"]')).find((b) => b.getAttribute('data-project') === pid) || null;
  const menuItem = (pid, action) =>
    Array.from(document.querySelectorAll('#app [data-action]')).find((b) => b.getAttribute('data-action') === action && b.getAttribute('data-project') === pid) || null;
  window.__ss = { txt, stateVersion, projectItem, shownProject, cards, inject, latest, dialog, stageRows, phaseSelect, dialogButton, dialogSnap, addButton, menuButton, menuItem };
}

// ---------------------------------------------------------------------------
// 段落共用
// ---------------------------------------------------------------------------

async function waitForFirstProjection(cdp, previewPort) {
  const start = Date.now();
  while (Date.now() - start < 8000) {
    const s = await cdp
      .eval(`(() => ({ lamp: !!document.querySelector('[data-region="topbar"] [data-runtime]'), v: window.__ss ? window.__ss.stateVersion() : null }))()`)
      .catch(() => null);
    if (s && s.lamp && s.v !== null) {
      const expected = await fetch(`http://127.0.0.1:${previewPort}/api/state`).then((r) => r.json()).catch(() => null);
      if (expected && s.v === String(expected.version)) {
        check(true, '第一份真投影已畫出（頂列有 [data-runtime] 且 #version 的 data-state-version 等於 /api/state 的 version）');
        return true;
      }
    }
    await sleep(50);
  }
  need(false, '逾時：第一份真投影已畫出');
  return false;
}

async function recordConsole(cdp) {
  const entries = [];
  cdp.onEvent('Runtime.consoleAPICalled', (p) => {
    entries.push({ at: Date.now(), level: p.type, text: (p.args || []).map((a) => (a.value !== undefined ? String(a.value) : a.description || '')).join(' ') });
  });
  cdp.onEvent('Runtime.exceptionThrown', (p) => {
    const d = p.exceptionDetails || {};
    entries.push({ at: Date.now(), level: 'exception', text: d.exception && d.exception.description ? d.exception.description : d.text });
  });
  await cdp.send('Runtime.enable');
  return entries;
}

async function openCockpit(label, opts = {}) {
  const ctx = { label, preview: null, chrome: null };
  try {
    ctx.preview = await startPreview(opts.env || {}, `preview-${label}`);
    ctx.chrome = await startChrome(pickPort(CDP_PORT_BASE, [ctx.preview.port]), 'about:blank', `chrome-${label}`, opts.windowSize);
    ctx.cdp = ctx.chrome.cdp;
    ctx.origin = `http://127.0.0.1:${ctx.preview.port}`;
    await ctx.cdp.send('Page.enable');
    await ctx.cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: `(${pageHelpers.toString()})();` });
    ctx.console = await recordConsole(ctx.cdp);
    await ctx.cdp.send('Page.navigate', { url: `${ctx.origin}/` });
    await waitForFirstProjection(ctx.cdp, ctx.preview.port);
    return ctx;
  } catch (e) {
    await closeCockpit(ctx);
    throw e;
  }
}

async function closeCockpit(ctx) {
  if (!ctx) return;
  await stopChrome(ctx.chrome, `chrome-${ctx.label}`);
  await stopPreview(ctx.preview, `preview-${ctx.label}`);
}

async function withCockpit(label, opts, body) {
  const ctx = await openCockpit(label, opts);
  try {
    await body(ctx);
  } finally {
    await closeCockpit(ctx);
  }
}

function noExceptions(ctx) {
  const ex = ctx.console.filter((e) => e.level === 'exception' || e.level === 'error');
  check(ex.length === 0, `頁面沒有例外或 console.error（實際 ${J(ex.slice(0, 3))}）`);
}

const stateVersion = (ctx) => ctx.cdp.run(() => window.__ss.stateVersion());
const cards = (ctx) => ctx.cdp.run(() => window.__ss.cards());

async function craft(ctx, mutate) {
  const base = await ctx.cdp.run(() => window.__ss.latest());
  need(!!base && Array.isArray(base.projects), '取得頁面目前的投影（window.cockpitLatestState）');
  const s = JSON.parse(JSON.stringify(base));
  s.version = Number(base.version) + 1;
  mutate(s);
  return s;
}
async function inject(ctx, state, label) {
  const from = await stateVersion(ctx);
  need(await ctx.cdp.run((st) => window.__ss.inject(st), state), `注入特製投影：${label}`);
  const ok = await ctx.cdp.poll((v) => window.__ss.stateVersion() === v, [String(state.version)], UI_TIMEOUT_MS);
  need(!!ok, `特製投影（${label}）已重畫（data-state-version ${from} → ${state.version}）`);
}

// 左欄點選 Project（真的滑鼠事件），等 Factory Floor 顯示它。
async function selectProject(ctx, id) {
  need(await ctx.cdp.clickEl((p) => window.__ss.projectItem(p), [id], `左欄 Project ${id}`), `點左欄 Project ${id}`);
  need(!!(await ctx.cdp.poll((p) => window.__ss.shownProject() === p, [id], UI_TIMEOUT_MS)), `Factory Floor 顯示 ${id}`);
}
// 頂列的語言切換鈕（真的點擊；setLang 寫 cockpit.lang 後整頁重新載入）→ 等新語言的首份投影。
async function toggleLanguage(ctx, expectLang) {
  need(
    await ctx.cdp.clickEl(() => document.querySelector('[data-action="toggle-language"]'), [], '頂列語言切換鈕'),
    '點頂列語言切換鈕'
  );
  const switched = await ctx.cdp.poll((lg) => window.cockpitI18n && window.cockpitI18n.lang === lg && !!window.__ss, [expectLang], 8000);
  need(!!switched, `重新載入後介面語言為 ${expectLang}`);
  await waitForFirstProjection(ctx.cdp, ctx.preview.port);
}

// --- 對比計算（WCAG 2.x 相對亮度；alpha 為 1 的不透明色）---
function luminance([r, g, b]) {
  const lin = (v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}
function contrast(fg, bg) {
  const a = luminance(fg);
  const b = luminance(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}
const ratioText = (n) => n.toFixed(2);

const byChange = (list, change) => list.find((c) => c.change === change) || null;

// 檢查兩張範例卡片的標示文字（lang 為 zh 或 en）。
function checkSampleCards(list, lang, label) {
  const auto = byChange(list, AUTO_CARD.change);
  const manual = byChange(list, MANUAL_CARD.change);
  need(!!auto && !!manual, `${label}：Factory Floor 有 ${AUTO_CARD.change} 與 ${MANUAL_CARD.change} 兩張帶同步標示的卡片（實際 ${J(list.map((c) => c.change))}）`);
  check(auto.hasSync && auto.progress === AUTO_CARD.progress, `${label}：${AUTO_CARD.change} 顯示 ${AUTO_CARD.progress}（實際 ${J(auto.progress)}）`);
  check(auto.mode === MODE_TEXT[lang].auto, `${label}：${AUTO_CARD.change} 的模式文字為 ${J(MODE_TEXT[lang].auto)}（實際 ${J(auto.mode)}）`);
  check(auto.syncMode === 'auto', `${label}：${AUTO_CARD.change} 的 data-sync-mode 為 auto（實際 ${J(auto.syncMode)}）`);
  check(manual.hasSync && manual.progress === MANUAL_CARD.progress, `${label}：${MANUAL_CARD.change} 顯示 ${MANUAL_CARD.progress}（實際 ${J(manual.progress)}）`);
  check(manual.mode === MODE_TEXT[lang].manual, `${label}：${MANUAL_CARD.change} 的模式文字為 ${J(MODE_TEXT[lang].manual)}（實際 ${J(manual.mode)}）`);
  check(manual.syncMode === 'manual', `${label}：${MANUAL_CARD.change} 的 data-sync-mode 為 manual（實際 ${J(manual.syncMode)}）`);
  for (const c of [auto, manual]) {
    check(c.syncTitle !== null && c.syncTitle.includes(c.change) && c.syncTitle.includes(c.progress), `${label}：${c.change} 的標示有 title 且含名稱與進度（實際 ${J(c.syncTitle)}）`);
  }
  return { auto, manual };
}

// ---------------------------------------------------------------------------
// 段落
// ---------------------------------------------------------------------------

// 自我測試：段落代號驗證與對比計算本身（不啟動任何行程）。
async function segSelfCheck() {
  const known = ['a/x', 'a/y', 'b/z'];
  check(J(parseSegmentArg('a/', known).codes) === J(['a/x', 'a/y']), '段落代號：前綴 a/ 選中 a/x、a/y');
  check(parseSegmentArg('nope', known).ok === false, '段落代號：拼錯的代號被拒絕');
  check(parseSegmentArg(' , ', known).ok === false, '段落代號：只有逗號被拒絕');
  check(Math.abs(contrast([0, 0, 0], [255, 255, 255]) - 21) < 1e-9, '對比計算：黑對白為 21:1');
  check(Math.abs(contrast([119, 119, 119], [255, 255, 255]) - 4.48) < 0.01, '對比計算：#777 對白約 4.48:1（低於 4.5）');
}

// spec「自動同步的卡片」「手動的卡片顯示較淡的手動標示」「沒有同步資訊的卡片」（繁中）。
async function segCardZh() {
  await withCockpit('zh', {}, async (ctx) => {
    await selectProject(ctx, 'demo-app');
    const list = await cards(ctx);
    const { auto, manual } = checkSampleCards(list, 'zh', '繁中');
    check(auto.syncText.includes(AUTO_CARD.change) && auto.syncText.includes('自動'), `繁中：${AUTO_CARD.change} 的一行標示含名稱與「自動」（實際 ${J(auto.syncText)}）`);
    check(manual.syncText.includes(MANUAL_CARD.change) && manual.syncText.includes('手動'), `繁中：${MANUAL_CARD.change} 的一行標示含名稱與「手動」（實際 ${J(manual.syncText)}）`);
    // 標示是節點的一行：位於標題之後、按鈕之前，不改變其他段落。
    for (const c of [auto, manual]) {
      check(
        /task-title/.test(c.childClasses[0]) && /task-sync/.test(c.childClasses[2]) && /task-actions/.test(c.childClasses[3]) && c.childClasses.length === 4,
        `繁中：${c.change} 節點由上而下為 標題、狀態、同步標示、按鈕（實際 ${J(c.childClasses)}）`
      );
    }
    // 沒有 sync 的 Project：整個 Factory Floor 一個標示都沒有。
    await selectProject(ctx, 'cockpit');
    const other = await cards(ctx);
    check(other.length > 0, `繁中：cockpit 有 task 節點可驗（實際 ${other.length}）`);
    check(other.every((c) => !c.anySyncInside), `繁中：sync 為 null 的卡片沒有同步標示（${other.length} 張）`);
    noExceptions(ctx);
  });
}

// spec「手動的卡片顯示較淡的手動標示」：計算後文字對比不低於 4.5:1，且手動比自動淡、不靠透明度。
async function segContrast() {
  await withCockpit('contrast', {}, async (ctx) => {
    await selectProject(ctx, 'demo-app');
    const list = await cards(ctx);
    const auto = byChange(list, AUTO_CARD.change);
    const manual = byChange(list, MANUAL_CARD.change);
    need(!!auto && !!manual && auto.hasSync && manual.hasSync, '前置：兩張帶標示的卡片存在');
    const ratios = {};
    for (const [name, c] of [['自動', auto], ['手動', manual]]) {
      ratios[name] = {};
      for (const part of ['change', 'progress', 'mode']) {
        const fg = c.colors[part];
        const r = fg ? contrast(fg, c.bg) : 0;
        ratios[name][part] = r;
        check(!!fg && fg[3] === 255, `${name}標示的 ${part} 文字顏色不透明（alpha 255，實際 ${J(fg)}）`);
        check(c.bg[3] === 255, `${name}卡片的背景是不透明色（實際 ${J(c.bg)}）`);
        check(r >= MIN_CONTRAST, `${name}標示的 ${part} 文字對比 ${ratioText(r)}:1 不低於 ${MIN_CONTRAST}:1`);
      }
      check(c.opacity === 1, `${name}標示及其祖先的 opacity 乘積為 1（不以透明度變淡，實際 ${c.opacity}）`);
    }
    check(
      ratios['手動'].mode < ratios['自動'].mode,
      `手動標示比自動標示淡（模式文字對比 手動 ${ratioText(ratios['手動'].mode)} < 自動 ${ratioText(ratios['自動'].mode)}）`
    );
    check(J(manual.colors.mode) !== J(auto.colors.mode), `手動與自動標示的計算顏色不同（自動 ${J(auto.colors.mode)}、手動 ${J(manual.colors.mode)}）`);
    check(auto.fontSize === '12px' && manual.fontSize === '12px', `標示字級為 --fs-meta 12px（實際 ${auto.fontSize}、${manual.fontSize}）`);
    noExceptions(ctx);
  });
}

// spec「英文介面」與 ui-language「英文介面沒有繁中字串」：標示為 Auto／Manual，卡片上沒有繁中字串。
async function segCardEn() {
  await withCockpit('en', {}, async (ctx) => {
    await toggleLanguage(ctx, 'en');
    await selectProject(ctx, 'demo-app');
    const list = await cards(ctx);
    const { auto, manual } = checkSampleCards(list, 'en', '英文');
    const cjk = /[\u3400-\u9fff]/;
    for (const c of [auto, manual]) {
      check(!cjk.test(c.syncText) && !cjk.test(c.syncTitle || ''), `英文：${c.change} 的標示文字與 title 不含繁中字元（實際 ${J(c.syncText)}／${J(c.syncTitle)}）`);
    }
    check(auto.mode !== '自動' && manual.mode !== '手動', '英文：模式文字不是繁中字典的字串');
    noExceptions(ctx);
  });
}

// spec「沒有同步資訊的卡片」「change 名稱不以 HTML 解讀」：以特製投影驗證。
async function segNullAndHtml() {
  await withCockpit('inject', {}, async (ctx) => {
    await selectProject(ctx, 'demo-app');
    // 把 fix-cache 卡片的 sync 改成 null；add-login 的 change 名稱改成會被當 HTML 解讀的字串。
    const evil = '<img src=x onerror=alert(1)>';
    await inject(
      ctx,
      await craft(ctx, (s) => {
        const demo = s.projects.find((p) => p.id === 'demo-app');
        for (const t of demo.tasks) {
          if (t.sync && t.sync.change === MANUAL_CARD.change) t.sync = null;
          else if (t.sync && t.sync.change === AUTO_CARD.change) t.sync.change = evil;
        }
      }),
      'fix-cache 的 sync 為 null、add-login 的名稱含 HTML'
    );
    const list = await cards(ctx);
    const withSync = list.filter((c) => c.hasSync);
    check(withSync.length === 1, `只剩一張卡片有同步標示（實際 ${withSync.length}）`);
    check(withSync[0] && withSync[0].change === evil, `change 名稱以原樣文字顯示（實際 ${J(withSync[0] && withSync[0].change)}）`);
    check(list.every((c) => !c.imgInside), '沒有建立任何 <img> 元素');
    const nulled = list.filter((c) => !c.hasSync);
    check(nulled.length === 1 && !nulled[0].anySyncInside, `sync 為 null 的那張卡片沒有同步標示（實際 ${J(nulled.map((c) => c.childClasses))}）`);
    check(
      nulled.length === 1 && nulled[0].childClasses.length === 3 && !nulled[0].childClasses.some((c) => /task-sync/.test(c)),
      `sync 為 null 的卡片節點外觀與沒有此功能時相同（只有標題、狀態、按鈕；實際 ${J(nulled[0] && nulled[0].childClasses)}）`
    );
    noExceptions(ctx);
  });
}

// spec「標示隨每一份新投影更新」與整頁重畫：同一份投影重畫、進度與模式變動、頻繁推送期間標示都在。
async function segRepaint() {
  await withCockpit('repaint', { env: { COCKPIT_PREVIEW_PUSH_MS: '100' } }, async (ctx) => {
    await selectProject(ctx, 'demo-app');
    const v0 = Number(await stateVersion(ctx));
    // 頻繁推送：觀察期間 version 一直前進，每次取樣兩張標示都在且文字不變。
    let samples = 0;
    let bad = 0;
    const seen = new Set();
    const start = Date.now();
    while (Date.now() - start < 1500) {
      const list = await cards(ctx);
      seen.add(await stateVersion(ctx));
      const a = byChange(list, AUTO_CARD.change);
      const m = byChange(list, MANUAL_CARD.change);
      samples += 1;
      if (!(a && a.hasSync && a.progress === AUTO_CARD.progress && a.mode === '自動' && m && m.hasSync && m.progress === MANUAL_CARD.progress && m.mode === '手動')) bad += 1;
      await sleep(30);
    }
    check(seen.size >= 3, `觀察期間投影 version 前進至少 3 次（實際 ${seen.size} 種，起點 ${v0}）`);
    check(samples >= 10 && bad === 0, `頻繁整頁重畫期間兩張標示每次取樣都在（${samples} 次取樣、${bad} 次異常）`);
    // 新投影改變進度與模式：標示跟著更新。
    await inject(
      ctx,
      await craft(ctx, (s) => {
        const demo = s.projects.find((p) => p.id === 'demo-app');
        for (const t of demo.tasks) {
          if (t.sync && t.sync.change === AUTO_CARD.change) {
            t.sync.checked = 4;
            t.sync.mode = 'manual';
          } else if (t.sync && t.sync.change === MANUAL_CARD.change) {
            t.sync.mode = 'auto';
          }
        }
      }),
      'add-login 4/8 改手動、fix-cache 改自動'
    );
    const list = await cards(ctx);
    const a = byChange(list, AUTO_CARD.change);
    const m = byChange(list, MANUAL_CARD.change);
    check(!!a && a.progress === '4/8' && a.mode === '手動' && a.syncMode === 'manual', `add-login 更新為 4/8、手動（實際 ${J(a && [a.progress, a.mode, a.syncMode])}）`);
    check(!!m && m.progress === '5/5' && m.mode === '自動' && m.syncMode === 'auto', `fix-cache 更新為 5/5、自動（實際 ${J(m && [m.progress, m.mode, m.syncMode])}）`);
    noExceptions(ctx);
  });
  // 靜止的整頁重畫（切換 Project 再切回、語言切換的重新載入）後標示仍在。
  await withCockpit('repaint-nav', {}, async (ctx) => {
    await selectProject(ctx, 'demo-app');
    await selectProject(ctx, 'cockpit');
    await selectProject(ctx, 'demo-app');
    const list = await cards(ctx);
    check(!!byChange(list, AUTO_CARD.change) && !!byChange(list, MANUAL_CARD.change), '切到別的 Project 再切回，兩張標示仍在');
    await inject(ctx, await craft(ctx, () => {}), '內容相同、version 加一的投影');
    const again = await cards(ctx);
    check(J(again.map((c) => [c.change, c.progress, c.mode])) === J(list.map((c) => [c.change, c.progress, c.mode])), '同內容的新投影整頁重畫後標示不變');
    noExceptions(ctx);
  });
}

// ---------------------------------------------------------------------------
// 段落：「編輯 stage」對話框的階段下拉與「加入」送出的 phases（openspec-stage-sync task 5.2）
// ---------------------------------------------------------------------------

const DEMO = { id: 'demo-app', name: 'Demo App', path: '/api/repo-projects/demo-app' };
const BILLING = { repo: 'd:\\work\\billing-api\\.git' };
const PHASE_VALUES = ['', 'plan', 'implement', 'review', 'complete'];
const PHASE_TEXT = {
  zh: ['不對應', '規劃', '實作', '審查', '完成'],
  en: ['None', 'Plan', 'Implement', 'Review', 'Complete'],
};
const DEFAULT_PHASES = ['plan', 'implement', 'review', 'complete'];
const DEFAULT_STAGES = { zh: ['規劃', '實作', '審查', '完成'], en: ['Plan', 'Implement', 'Review', 'Complete'] };
const ARROW_UP = ['ArrowUp', 'ArrowUp', 38, ''];
const ARROW_DOWN = ['ArrowDown', 'ArrowDown', 40, ''];

const dsnap = (ctx) => ctx.cdp.run(() => window.__ss.dialogSnap());
const writesOf = (ctx, method, p) => ctx.preview.writes.filter((w) => w.method === method && w.path === p);
async function waitWrite(ctx, method, p, n, timeoutMs = UI_TIMEOUT_MS) {
  const start = Date.now();
  for (;;) {
    const list = writesOf(ctx, method, p);
    if (list.length >= n) return list[n - 1];
    if (Date.now() - start >= timeoutMs) return null;
    await sleep(50);
  }
}
const demoProject = (st) => st.projects.find((x) => x.id === DEMO.id);
// 注入 demo-app 的 stages 與 stage_phases。
async function setDemo(ctx, stages, phases, label) {
  await inject(
    ctx,
    await craft(ctx, (st) => {
      const d = demoProject(st);
      d.stages = stages.slice();
      d.stage_phases = phases.slice();
    }),
    label
  );
}
// 真的滑鼠：點「⋯」→ 點「編輯 stage」。
async function openStagesDialog(ctx) {
  need(await ctx.cdp.clickEl((p) => window.__ss.menuButton(p), [DEMO.id], `${DEMO.id} 的「⋯」`), `點 ${DEMO.id} 的「⋯」`);
  need(
    await ctx.cdp.clickEl((p) => window.__ss.menuItem(p, 'project-edit-stages'), [DEMO.id], '選單項目「編輯 stage」'),
    '點選單項目「編輯 stage」'
  );
  need(
    !!(await ctx.cdp.poll(() => { const d = window.__ss.dialogSnap(); return !!d && d.open && d.kind === 'stages'; }, [], UI_TIMEOUT_MS)),
    '編輯 stage 對話框開啟'
  );
}
async function clickDialog(ctx, op, row = null, desc = null) {
  const label = desc || `對話框的 ${op}${row === null ? '' : `（第 ${row + 1} 列）`}`;
  need(await ctx.cdp.clickEl((o, r) => window.__ss.dialogButton(o, r), [op, row], label), `點${label}`);
}
// 鍵盤操作某列的階段下拉：焦點移到它（程式 focus，等同 Tab 到達），再送真的方向鍵。
async function focusPhase(ctx, row) {
  need(
    await ctx.cdp.run((i) => { const el = window.__ss.phaseSelect(i); if (!el) return false; el.focus(); return document.activeElement === el; }, row),
    `第 ${row + 1} 列的階段下拉取得焦點`
  );
}
async function arrow(ctx, which) {
  await ctx.cdp.pressKey(...which);
  await sleep(80);
}
async function waitDialogClosed(ctx) {
  return !!(await ctx.cdp.poll(() => { const d = window.__ss.dialogSnap(); return !!d && !d.open; }, [], UI_TIMEOUT_MS));
}

// spec「編輯 stage 對話框的階段下拉」（繁中與英文）：初始值取自 stage_phases、新增的列為不對應、選項順序固定。
async function segDialogInitial() {
  await withCockpit('dlg-init', {}, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await setDemo(ctx, ['Plan', 'Build', 'Done'], ['plan', null, 'complete'], 'stages 為 Plan、Build、Done，stage_phases 為 [plan, null, complete]');
    await openStagesDialog(ctx);
    let d = await dsnap(ctx);
    check(J(d.stages) === J(['Plan', 'Build', 'Done']), `對話框列出 Plan、Build、Done（實際 ${J(d.stages)}）`);
    check(J(d.tags) === J(['SELECT', 'SELECT', 'SELECT']), `每列各有一個 <select>（實際 ${J(d.tags)}）`);
    check(J(d.phases) === J(['plan', '', 'complete']), `三列下拉的值依序為 plan、（不對應）、complete（實際 ${J(d.phases)}）`);
    check(J(d.shown) === J(['規劃', '不對應', '完成']), `三列下拉顯示「規劃」「不對應」「完成」（實際 ${J(d.shown)}）`);
    const want = PHASE_VALUES.map((v, i) => [v, PHASE_TEXT.zh[i]]);
    check(d.options.every((o) => J(o) === J(want)), `每個下拉的選項依序為 不對應、規劃、實作、審查、完成（實際 ${J(d.options[0])}）`);
    check(d.labels.every((l) => typeof l === 'string' && l.trim() !== ''), `每個下拉有無障礙名稱（實際 ${J(d.labels)}）`);
    check(new Set(d.labels).size === d.labels.length, `各列下拉的無障礙名稱彼此不同（實際 ${J(d.labels)}）`);
    check(!d.inApp, '對話框在 #app 之外');
    await clickDialog(ctx, 'add', null, '「新增 stage」');
    d = await dsnap(ctx);
    check(d.phases.length === 4 && d.phases[3] === '' && d.shown[3] === '不對應', `新增的列下拉初始為「不對應」（實際 ${J(d.phases)}／${J(d.shown)}）`);
    check(J(d.phases.slice(0, 3)) === J(['plan', '', 'complete']), `新增列不動既有列的選擇（實際 ${J(d.phases)}）`);
    noExceptions(ctx);
  });
  // stage_phases 比 stages 短或缺席（舊投影、手寫 project）：缺的視為不對應，不丟例外。
  await withCockpit('dlg-init-short', {}, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await inject(
      ctx,
      await craft(ctx, (st) => {
        const d = demoProject(st);
        d.stages = ['Plan', 'Build', 'Done'];
        d.stage_phases = ['plan'];
      }),
      'stage_phases 比 stages 短'
    );
    await openStagesDialog(ctx);
    const d = await dsnap(ctx);
    check(J(d.phases) === J(['plan', '', '']), `缺的對應視為不對應（實際 ${J(d.phases)}）`);
    noExceptions(ctx);
  });
  await withCockpit('dlg-init-en', {}, async (ctx) => {
    await toggleLanguage(ctx, 'en');
    await selectProject(ctx, DEMO.id);
    await openStagesDialog(ctx);
    const d = await dsnap(ctx);
    check(J(d.phases) === J(['plan', 'implement']), `英文：fixture 的 Plan、Build 下拉值為 plan、implement（實際 ${J(d.phases)}）`);
    const want = PHASE_VALUES.map((v, i) => [v, PHASE_TEXT.en[i]]);
    check(d.options.every((o) => J(o) === J(want)), `英文：下拉選項為 None、Plan、Implement、Review、Complete，value 仍是 plan 等字串（實際 ${J(d.options[0])}）`);
    const cjk = /[\u3400-\u9fff]/;
    check(!cjk.test(d.dialogText) && d.labels.every((l) => !cjk.test(l || '')), `英文：對話框文字與下拉的無障礙名稱不含繁中字元（實際 ${J(d.labels)}）`);
    noExceptions(ctx);
  });
}

// spec「選到已被使用的階段時他列改回不對應」與「編輯 stage」本體：以鍵盤操作下拉；PATCH 每列帶 phase。
async function segDialogUniqueAndBody() {
  await withCockpit('dlg-body', {}, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await setDemo(ctx, ['Plan', 'Build'], ['plan', null], 'Plan 對應 plan、Build 不對應');
    await openStagesDialog(ctx);
    await focusPhase(ctx, 1);
    await arrow(ctx, ARROW_DOWN); // 不對應 → 規劃
    let d = await dsnap(ctx);
    check(J(d.phases) === J(['', 'plan']), `Build 選「規劃」後 Plan 自動改回不對應（實際 ${J(d.phases)}）`);
    check(J(d.shown) === J(['不對應', '規劃']), `畫面顯示 Plan 為「不對應」、Build 為「規劃」（實際 ${J(d.shown)}）`);
    check(!!d.active && d.active.tag === 'SELECT' && d.active.row === 1, `改選後焦點仍在 Build 的下拉（實際 ${J(d.active)}）`);
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w = await waitWrite(ctx, 'PATCH', DEMO.path, 1);
    need(w !== null, '按「儲存」後服務收到 PATCH');
    const want = { stages: [{ name: 'Plan', from: 'Plan', phase: null }, { name: 'Build', from: 'Build', phase: 'plan' }] };
    check(w.body === J(want), `本體中只有 Build 的 phase 為 plan、Plan 的 phase 為 null（期望 ${J(want)}，實際 ${w.body}）`);
    check(await waitDialogClosed(ctx), '成功後對話框關閉');

    // 另一組：四列、鍵盤用 ArrowUp 換手與回到不對應、新增一列；每列都帶 phase（含 null）。
    await setDemo(ctx, ['Plan', 'Implement', 'Review', 'Done'], ['plan', 'implement', null, 'complete'], '四個 stage 的對應');
    await openStagesDialog(ctx);
    d = await dsnap(ctx);
    check(J(d.phases) === J(['plan', 'implement', '', 'complete']), `開啟時下拉值取自 stage_phases（實際 ${J(d.phases)}）`);
    await focusPhase(ctx, 1);
    await arrow(ctx, ARROW_UP); // implement → plan：Plan 列被換成不對應
    d = await dsnap(ctx);
    check(J(d.phases) === J(['', 'plan', '', 'complete']), `Implement 列改選 plan：原本的 Plan 列改回不對應（實際 ${J(d.phases)}）`);
    await arrow(ctx, ARROW_UP); // plan → 不對應
    d = await dsnap(ctx);
    check(J(d.phases) === J(['', '', '', 'complete']), `改回不對應不影響其他列（實際 ${J(d.phases)}）`);
    await clickDialog(ctx, 'add', null, '「新增 stage」');
    await ctx.cdp.send('Input.insertText', { text: 'Ship' });
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w2 = await waitWrite(ctx, 'PATCH', DEMO.path, 2);
    const want2 = {
      stages: [
        { name: 'Plan', from: 'Plan', phase: null },
        { name: 'Implement', from: 'Implement', phase: null },
        { name: 'Review', from: 'Review', phase: null },
        { name: 'Done', from: 'Done', phase: 'complete' },
        { name: 'Ship', from: null, phase: null },
      ],
    };
    check(w2 !== null && w2.body === J(want2), `每一列都帶 phase，新增的列為 null（期望 ${J(want2)}，實際 ${w2 ? w2.body : '沒有請求'}）`);
    check(writesOf(ctx, 'PATCH', DEMO.path).length === 2, `共 2 筆 PATCH（實際 ${writesOf(ctx, 'PATCH', DEMO.path).length}）`);
    noExceptions(ctx);
  });
}

// spec「階段對應在別處被改過時不送出」：只有 stage_phases 變（stages 沒變）也算過期。
async function segDialogStalePhases() {
  await withCockpit('dlg-stale', {}, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await setDemo(ctx, ['Plan', 'Build'], ['plan', null], '開啟前：stage_phases 為 [plan, null]');
    await openStagesDialog(ctx);
    await setDemo(ctx, ['Plan', 'Build'], [null, 'plan'], '期間 stage_phases 變成 [null, plan]（stages 沒變）');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    await sleep(500);
    const d = await dsnap(ctx);
    check(writesOf(ctx, 'PATCH', DEMO.path).length === 0, `服務沒有收到任何 PATCH（實際 ${writesOf(ctx, 'PATCH', DEMO.path).length}）`);
    check(d.open && d.error !== null && /別處/.test(d.error), `對話框不關並顯示需重新開啟的提示（實際 ${J(d.error)}）`);
    check(J(d.phases) === J(['plan', '']), `已編輯的內容保留（下拉仍是開啟時的 plan、不對應；實際 ${J(d.phases)}）`);
    noExceptions(ctx);
  });
  // 對照組：stages 與 stage_phases 都沒變（只有別的欄位變），照常送出。
  await withCockpit('dlg-fresh', {}, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await openStagesDialog(ctx);
    await inject(ctx, await craft(ctx, (st) => { demoProject(st).name = 'Demo App 2'; }), '只改名稱的新投影');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w = await waitWrite(ctx, 'PATCH', DEMO.path, 1);
    check(w !== null, 'stages 與 stage_phases 都沒變時照常送出 PATCH');
    const want = { stages: [{ name: 'Plan', from: 'Plan', phase: 'plan' }, { name: 'Build', from: 'Build', phase: 'implement' }] };
    check(w !== null && w.body === J(want), `fixture 的對應原樣帶回（期望 ${J(want)}，實際 ${w ? w.body : '沒有請求'}）`);
    noExceptions(ctx);
  });
}

// 對話框開著期間整頁重畫（頻繁推送與注入）：下拉選擇保留；結構改變（上移）重建後選擇跟著列走（狀態在模組變數，不只在 DOM）。
async function segDialogRepaint() {
  await withCockpit('dlg-repaint', { env: { COCKPIT_PREVIEW_PUSH_MS: '100' } }, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await openStagesDialog(ctx);
    await focusPhase(ctx, 1);
    await arrow(ctx, ARROW_UP); // Build：implement → plan，Plan 列改回不對應
    let d = await dsnap(ctx);
    need(J(d.phases) === J(['', 'plan']), `前置：Build 選了規劃、Plan 為不對應（實際 ${J(d.phases)}）`);
    const v0 = Number(await stateVersion(ctx));
    const seen = new Set();
    let bad = 0;
    const start = Date.now();
    while (Date.now() - start < 1500) {
      seen.add(await stateVersion(ctx));
      const x = await dsnap(ctx);
      if (!(x && x.open && J(x.phases) === J(['', 'plan']) && x.active && x.active.tag === 'SELECT' && x.active.row === 1)) bad += 1;
      await sleep(30);
    }
    check(seen.size >= 3, `觀察期間投影 version 前進至少 3 次（實際 ${seen.size} 種，起點 ${v0}）`);
    check(bad === 0, `頻繁整頁重畫期間下拉的選擇與焦點每次取樣都在（${bad} 次異常）`);
    // 結構改變：Build 上移 → 重建列，選擇跟著列走。
    await clickDialog(ctx, 'up', 1, 'Build 的「上移」');
    d = await dsnap(ctx);
    check(
      J(d.stages) === J(['Build', 'Plan']) && J(d.phases) === J(['plan', '']),
      `Build 上移後重建：Build 仍是規劃、Plan 仍是不對應（實際 ${J(d.stages)}／${J(d.phases)}）`
    );
    // 重建後再整頁重畫，仍保留。
    await inject(ctx, await craft(ctx, () => {}), '內容相同、version 加一的投影');
    d = await dsnap(ctx);
    check(d.open && J(d.phases) === J(['plan', '']), `重建後再整頁重畫選擇仍在（實際 ${J(d.phases)}）`);
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w = await waitWrite(ctx, 'PATCH', DEMO.path, 1);
    const want = { stages: [{ name: 'Build', from: 'Build', phase: 'plan' }, { name: 'Plan', from: 'Plan', phase: null }] };
    check(w !== null && w.body === J(want), `送出的本體反映重畫與上移後的選擇（期望 ${J(want)}，實際 ${w ? w.body : '沒有請求'}）`);
    noExceptions(ctx);
  });
}

// 版面：階段下拉不撐破對話框。一般寬度時名稱、下拉、按鈕群同一行；窄視窗（360 寬）放不下就換行，但每個元件都在列內、
// 對話框不超出視窗也不橫向溢位。
async function segDialogLayout() {
  const inside = (c, row) => !!c && c.l >= row.l - 1 && c.r <= row.r + 1;
  await withCockpit('dlg-layout', {}, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await openStagesDialog(ctx);
    const d = await dsnap(ctx);
    check(d.rects.length === 2 && d.rects.every((x) => x.select && x.select.w > 40), `每列的下拉有寬度（實際 ${J(d.rects.map((x) => x.select && x.select.w))}）`);
    const sameLine = d.rects.every((x) => x.input.t < x.actions.b && x.actions.t < x.input.b && x.select.t < x.actions.b && x.actions.t < x.select.b);
    check(sameLine, `一般寬度時名稱、下拉、按鈕群在同一行（實際 ${J(d.rects)}）`);
    check(d.rects.every((x) => inside(x.input, x.row) && inside(x.select, x.row) && inside(x.actions, x.row)), '一般寬度時各元件都在列內');
    check(d.rects.every((x) => x.input.w >= 80), `一般寬度時名稱輸入框至少 80px 寬（實際 ${J(d.rects.map((x) => x.input.w))}）`);
    check(!d.dialogRect.overflowX, '一般寬度時對話框沒有橫向溢位');
    noExceptions(ctx);
  });
  await withCockpit('dlg-layout-narrow', { windowSize: '360,800' }, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await openStagesDialog(ctx);
    const d = await dsnap(ctx);
    check(d.dialogRect.l >= 0 && d.dialogRect.r <= d.viewportW + 1, `360 寬時對話框在視窗內（${J(d.dialogRect)}，視窗 ${d.viewportW}）`);
    check(!d.dialogRect.overflowX, '360 寬時對話框沒有橫向溢位');
    check(d.rects.every((x) => inside(x.input, x.row) && inside(x.select, x.row) && inside(x.actions, x.row)), `360 寬時各元件都在列內（實際 ${J(d.rects)}）`);
    check(d.rects.every((x) => x.input.w >= 60 && x.select.w >= 40), `360 寬時名稱輸入框與下拉仍有可用寬度（實際 ${J(d.rects.map((x) => [x.input.w, x.select.w]))}）`);
    noExceptions(ctx);
  });
}

// 對話框既有行為不退化：Tab 能到達下拉且循環仍包住對話框；Esc 取消不送請求、焦點回「⋯」、再開不留上次的選擇。
async function segDialogKeyboard() {
  await withCockpit('dlg-kbd', {}, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await openStagesDialog(ctx);
    // 從第一列的名稱輸入框起按 Tab：下一個停點是同一列的階段下拉。
    need(
      await ctx.cdp.run(() => { const el = window.__ss.stageRows()[0].querySelector('input.stage-name'); el.focus(); return document.activeElement === el; }),
      '第一列名稱輸入框取得焦點'
    );
    await ctx.cdp.pressKey('Tab', 'Tab', 9, '');
    await sleep(80);
    let d = await dsnap(ctx);
    check(!!d.active && d.active.tag === 'SELECT' && d.active.row === 0, `名稱之後的下一個 Tab 停點是同一列的階段下拉（實際 ${J(d.active)}）`);
    // 最後一個元素（儲存）按 Tab 回到第一個；第一個按 Shift+Tab 到最後一個：循環仍包住對話框。
    await ctx.cdp.run(() => { window.__ss.dialogButton('submit', null).focus(); });
    await ctx.cdp.pressKey('Tab', 'Tab', 9, '');
    await sleep(80);
    d = await dsnap(ctx);
    check(!!d.active && d.active.row === 0 && /stage-name/.test(d.active.cls), `最後一個元素按 Tab 回到第一列名稱輸入框（實際 ${J(d.active)}）`);
    await ctx.cdp.pressKey('Tab', 'Tab', 9, '', 8);
    await sleep(80);
    d = await dsnap(ctx);
    check(!!d.active && d.active.op === 'submit', `第一個元素按 Shift+Tab 回到最後一個「儲存」（實際 ${J(d.active)}）`);
    // Esc 取消：不送請求，對話框關閉，焦點回「⋯」。
    await ctx.cdp.pressKey('Escape', 'Escape', 27, '');
    check(await waitDialogClosed(ctx), 'Esc 關閉對話框');
    const back = await ctx.cdp.run(() => {
      const a = document.activeElement;
      return a ? { action: a.getAttribute('data-action'), project: a.getAttribute('data-project') } : null;
    });
    check(!!back && back.action === 'project-menu' && back.project === DEMO.id, `Esc 後焦點回到 ${DEMO.id} 的「⋯」（實際 ${J(back)}）`);
    check(ctx.preview.writes.length === 0, `Esc 取消沒有送出任何請求（實際 ${ctx.preview.writes.length}）`);
    // 改了下拉後取消再開：不留上次的選擇。
    await openStagesDialog(ctx);
    await focusPhase(ctx, 1);
    await arrow(ctx, ARROW_UP);
    await ctx.cdp.pressKey('Escape', 'Escape', 27, '');
    await waitDialogClosed(ctx);
    await openStagesDialog(ctx);
    d = await dsnap(ctx);
    check(J(d.phases) === J(['plan', 'implement']), `取消後再開，下拉回到投影的對應（實際 ${J(d.phases)}）`);
    noExceptions(ctx);
  });
}

// spec「按加入送出預設 stages」：本體含 phases（與介面語言無關）。
async function segAddSendsPhases() {
  await withCockpit('add-phases', {}, async (ctx) => {
    need(await ctx.cdp.clickEl((r) => window.__ss.addButton(r), [BILLING.repo], '「加入」billing-api'), '點 billing-api 的「加入」');
    const w = await waitWrite(ctx, 'POST', '/api/repo-projects', 1);
    need(w !== null, '按「加入」後服務收到 POST /api/repo-projects');
    const want = { repo: BILLING.repo, stages: DEFAULT_STAGES.zh, phases: DEFAULT_PHASES };
    check(w.body === J(want), `繁中：本體恰為 ${J(want)}（實際 ${w.body}）`);
    noExceptions(ctx);
  });
  await withCockpit('add-phases-en', {}, async (ctx) => {
    await toggleLanguage(ctx, 'en');
    need(await ctx.cdp.clickEl((r) => window.__ss.addButton(r), [BILLING.repo], '「加入」billing-api'), '英文：點 billing-api 的「加入」');
    const w = await waitWrite(ctx, 'POST', '/api/repo-projects', 1);
    need(w !== null, '英文：按「加入」後服務收到 POST /api/repo-projects');
    const want = { repo: BILLING.repo, stages: DEFAULT_STAGES.en, phases: DEFAULT_PHASES };
    check(w.body === J(want), `英文：stages 為英文、phases 不變（期望 ${J(want)}，實際 ${w.body}）`);
    noExceptions(ctx);
  });
}

// 5.3 設計審核：對話框說明文字點出下拉是「OpenSpec 階段對應」（中英）、同列名稱輸入框與下拉等高、「不對應」比已選階段暗但對比仍 >= 4.5。
const HINT_TEXT = {
  zh: '順序就是 Factory Floor 由左到右的欄。被刪除的 stage 裡的 task 會移到第一個 stage。每列的下拉選擇這個 stage 對應的 OpenSpec 階段，卡片會依 OpenSpec 進度自動移到對應的 stage。',
  en: 'The order is the Factory Floor columns from left to right. Tasks in a deleted stage move to the first stage. Each row\'s dropdown picks the OpenSpec phase this stage maps to; cards move to the matching stage automatically as OpenSpec progresses.',
};
async function segDialogHintAndLook() {
  await withCockpit('dlg-look', {}, async (ctx) => {
    await selectProject(ctx, DEMO.id);
    await setDemo(ctx, ['Plan', 'Build', 'Done'], ['plan', null, 'complete'], 'stages 為 Plan、Build、Done，stage_phases 為 [plan, null, complete]');
    await openStagesDialog(ctx);
    const d = await dsnap(ctx);
    check(d.hint === HINT_TEXT.zh, `繁中：說明文字含 OpenSpec 階段對應的說明（期望 ${J(HINT_TEXT.zh)}，實際 ${J(d.hint)}）`);
    check(!/完成/.test(d.hint), '繁中：說明文字沒有「完成」二字（避免與 HERDR done 混淆）');
    check(
      d.rects.length === 3 && d.rects.every((x) => Math.abs(x.input.h - x.select.h) <= 1),
      `同列名稱輸入框與階段下拉等高（差 <= 1px；實際 ${J(d.rects.map((x) => [x.input.h, x.select.h]))}）`
    );
    check(
      d.rects.every((x) => Math.abs(x.input.b - x.select.b) <= 1 && Math.abs(x.input.t - x.select.t) <= 1),
      `同列名稱輸入框與下拉的上下緣對齊（差 <= 1px；實際 ${J(d.rects.map((x) => [x.input.t, x.input.b, x.select.t, x.select.b]))}）`
    );
    const none = d.selectColors[1];
    const picked = d.selectColors[0];
    check(!!none && !!picked && J(none.fg) !== J(picked.fg), `「不對應」的文字顏色與已選階段不同（不對應 ${J(none && none.fg)}、已選 ${J(picked && picked.fg)}）`);
    if (none && picked) {
      const rn = contrast(none.fg, none.bg);
      const rp = contrast(picked.fg, picked.bg);
      check(none.fg[3] === 255 && none.bg[3] === 255, `「不對應」的文字與背景都是不透明色（實際 ${J(none)}）`);
      check(rn >= MIN_CONTRAST, `「不對應」文字對下拉背景的對比 ${ratioText(rn)}:1 不低於 ${MIN_CONTRAST}:1`);
      check(rn < rp, `「不對應」比已選階段暗（對比 ${ratioText(rn)} < ${ratioText(rp)}）`);
    }
    // 改選一個階段後，該列不再是淡色（:has(option[value=""]:checked) 隨選擇更新）。
    await focusPhase(ctx, 1);
    await arrow(ctx, ARROW_DOWN);
    const d2 = await dsnap(ctx);
    check(d2.phases[1] !== '' && J(d2.selectColors[1].fg) === J(picked.fg), `改選階段後該列文字恢復一般顏色（值 ${J(d2.phases[1])}，顏色 ${J(d2.selectColors[1].fg)}）`);
    noExceptions(ctx);
  });
  await withCockpit('dlg-look-en', {}, async (ctx) => {
    await toggleLanguage(ctx, 'en');
    await selectProject(ctx, DEMO.id);
    await openStagesDialog(ctx);
    const d = await dsnap(ctx);
    check(d.hint === HINT_TEXT.en, `英文：說明文字含 OpenSpec phase 的說明（期望 ${J(HINT_TEXT.en)}，實際 ${J(d.hint)}）`);
    check(!/[㐀-鿿]/.test(d.hint), '英文：說明文字不含繁中字元');
    noExceptions(ctx);
  });
}

// 5.3 設計審核：超長 change 名稱最多兩行（title 仍帶完整名稱），短名稱不受影響。
async function segCardLongChange() {
  await withCockpit('long-change', {}, async (ctx) => {
    await selectProject(ctx, 'demo-app');
    const long = 'add-extremely-long-openspec-change-name-'.repeat(8) + 'END';
    await inject(
      ctx,
      await craft(ctx, (s) => {
        const demo = s.projects.find((p) => p.id === 'demo-app');
        for (const t of demo.tasks) if (t.sync && t.sync.change === AUTO_CARD.change) t.sync.change = long;
      }),
      'add-login 的 change 名稱改成超長字串'
    );
    const list = await cards(ctx);
    const c = byChange(list, long);
    need(!!c && !!c.changeBox, '前置：超長名稱的卡片存在');
    const lines = c.changeBox.h / c.changeBox.lh;
    check(c.changeBox.lh > 0 && lines <= 2.05, `超長名稱的標示不超過兩行（高 ${c.changeBox.h}px、行高 ${c.changeBox.lh}px，約 ${lines.toFixed(2)} 行）`);
    check(c.changeBox.clipped, '超長名稱確實被截斷（內容高於可視高）');
    check(c.syncTitle !== null && c.syncTitle.includes(long), '整列 title 仍帶完整名稱');
    check(c.change === long, '文字內容仍是完整名稱（只是視覺截斷）');
    const short = byChange(list, MANUAL_CARD.change);
    check(!!short && short.changeBox.h / short.changeBox.lh <= 1.05 && !short.changeBox.clipped, `短名稱仍是單行且未截斷（實際 ${J(short && short.changeBox)}）`);
    noExceptions(ctx);
  });
}

const SEGMENTS = [
  { code: 'self/段落代號與對比計算', fn: segSelfCheck, self: true },
  { code: 'card/繁中標示與無 sync 的卡片', fn: segCardZh },
  { code: 'card/手動標示較淡且對比不低於 4.5', fn: segContrast },
  { code: 'card/英文介面', fn: segCardEn },
  { code: 'card/sync 為 null 與名稱不以 HTML 解讀', fn: segNullAndHtml },
  { code: 'card/重畫後標示仍在並隨投影更新', fn: segRepaint },
  { code: 'dialog/階段下拉的初值與選項', fn: segDialogInitial },
  { code: 'dialog/唯一性自動切換與送出本體每列帶 phase', fn: segDialogUniqueAndBody },
  { code: 'dialog/過期檢查涵蓋 stage_phases', fn: segDialogStalePhases },
  { code: 'dialog/跨整頁重畫保留下拉選擇', fn: segDialogRepaint },
  { code: 'dialog/鍵盤操作與既有焦點行為', fn: segDialogKeyboard },
  { code: 'dialog/版面不被下拉撐破', fn: segDialogLayout },
  { code: 'add/加入送出預設四站與 phases', fn: segAddSendsPhases },
  { code: 'dialog/說明文字與下拉外觀', fn: segDialogHintAndLook },
  { code: 'card/超長 change 名稱最多兩行', fn: segCardLongChange },
];

async function main() {
  const known = SEGMENTS.map((s) => s.code);
  const parsed =
    POSITIONAL.length > 1
      ? { ok: false, message: `只接受一個段落參數（逗號分隔），實際 ${POSITIONAL.length} 個：${JSON.stringify(POSITIONAL)}` }
      : parseSegmentArg(SEGMENT_ARG, known);
  if (!parsed.ok) {
    console.error(`FAIL ${parsed.message}`);
    console.log('RESULT: FAIL (段落代號)');
    process.exitCode = 2;
    return;
  }
  const only = parsed.codes;
  if (!fs.existsSync(CHROME)) throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  const busy = [PREVIEW_PORT_BASE, CDP_PORT_BASE].filter(isPortListening);
  const leftovers = ourChromePids();
  if (busy.length > 0 || leftovers.length > 0) {
    console.error(
      `FAIL 開跑前 ${busy.length ? `127.0.0.1:${busy.join('、')} 已有人 LISTEN` : ''}${busy.length && leftovers.length ? '；' : ''}${
        leftovers.length ? `已有 user-data-dir 含 ${CHROME_UDD_PREFIX} 的 Chrome（PID ${JSON.stringify(leftovers)}，可能是上一次本腳本的殘留或另一份本腳本正在跑）` : ''
      }。本腳本不動別人的行程，請先確認後再跑`
    );
    console.log('RESULT: FAIL (環境)');
    process.exitCode = 2;
    return;
  }
  const others = runningUiPreviewPids();
  if (others.length > 0) log(`注意：已有其他 ui_preview.exe 在執行（PID ${JSON.stringify(others)}），不是本腳本開的，不處理`);
  if (isPortListening(7770)) log('注意：127.0.0.1:7770 有人 LISTEN（不是本腳本，不處理）');

  const summary = [];
  for (const seg of SEGMENTS) {
    if (only && !only.includes(seg.code)) continue;
    CURRENT_SEGMENT = seg.code;
    log(`=== ${seg.code} ===`);
    const before = failures.length;
    try {
      await seg.fn();
    } catch (e) {
      if (e instanceof SegmentAbort) log(`${seg.code}：前置條件不成立，本段中止`);
      else check(false, `${seg.code} 段中止：${e.message}`);
    }
    const segFails = failures.slice(before);
    summary.push({ code: seg.code, self: !!seg.self, fails: segFails.length, first: segFails[0] ? segFails[0].label : null });
  }

  CURRENT_SEGMENT = '收尾';
  const beforeFinal = failures.length;
  finalSweep();
  for (const port of [...OUR_PORTS].sort()) check(!isPortListening(port), `結束後本腳本用過的 port ${port} 沒有 LISTENING 的行程`);
  // 殘留 = 同 PID 且建立時間與 spawn 當下相同；PID 被別的程式重用（建立時間不同）不算。
  const unknownIdentity = OUR_IDENTITIES.filter((e) => e.created === null);
  const alive = OUR_IDENTITIES.filter((e) => e.created !== null && identityAlive(e)).map((e) => e.pid);
  check(alive.length === 0, `結束後本腳本 spawn 過的行程都已不存在（殘留 PID＋建立時間相符者 ${JSON.stringify(alive)}）`);
  if (unknownIdentity.length > 0) log(`注意：${unknownIdentity.length} 個行程在 spawn 當下查不到建立時間（PID ${JSON.stringify(unknownIdentity.map((e) => e.pid))}），收尾無法以身分判定，僅靠 port 與 exit 事件確認`);
  check(ourChromePids().length === 0, `結束後沒有殘留的 headless Chrome（user-data-dir 含 ${CHROME_UDD_PREFIX}）`);
  const leftDirs = [...PREVIEW_TEMP_ROOTS].filter((d) => fs.existsSync(d));
  check(leftDirs.length === 0, `結束後本腳本開過的 ui_preview 暫存目錄都已刪除（殘留 ${JSON.stringify(leftDirs)}）`);
  const hygieneFails = failures.length - beforeFinal;

  console.log('');
  console.log('=== 段落彙總 ===');
  for (const s of summary) {
    const tag = s.fails === 0 ? 'PASS' : `FAIL(${s.fails})`;
    console.log(`${tag.padEnd(9)} ${s.self ? '[自我測試] ' : ''}${s.code}${s.first ? ` — ${s.first.slice(0, 200)}` : ''}`);
  }
  console.log(`${(hygieneFails === 0 ? 'PASS' : `FAIL(${hygieneFails})`).padEnd(9)} [收尾衛生]`);
  const okCount = summary.filter((s) => s.fails === 0).length;
  console.log(`段落：${okCount}/${summary.length} PASS；收尾衛生：${hygieneFails === 0 ? 'PASS' : 'FAIL'}`);
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
