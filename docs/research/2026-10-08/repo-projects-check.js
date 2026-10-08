// repo-projects-check.js：repo-projects 前端驗收腳本（repo-projects task 5.1 建立；5.2、5.3 往上加段落）。
// headless Chrome＋raw CDP。啟動、收尾、行程所有權模型、段落代號與輸出格式沿用
// docs/research/2026-10-04/split-check.js（startPreview／startChrome／killTree＋tasklist 收尾判準、
// 「還握著 ChildProcess 且沒觀察到 exit 才終止」、parseSegmentArg、打錯段名 exit 2）。
//
// 對 `cockpit --example ui_preview` 驗 openspec/changes/repo-projects/specs/cockpit-dashboard/spec.md
// 「Project 切換」中屬於前端的 scenario：偵測到的 repo 區、「加入」送出的請求、加入後自動選定、空狀態文字。
//
// 用法（repo 根；先 `cargo build -p cockpit --example ui_preview`——前端資產內嵌在執行檔裡，改了
// cockpit/assets/ 沒重建就是驗舊版）：
//   node docs/research/2026-10-08/repo-projects-check.js                     # 全部段落
//   node docs/research/2026-10-08/repo-projects-check.js "cockpit-dashboard/" # 只跑指定段落（逗號分隔；`<前綴>/` 選該前綴全部）
// 段落代號拼錯、空字串或只有逗號 → 印 `RESULT: FAIL (段落代號)`、exit 2，不啟動任何行程。
// 段落代號與每段驗什麼見檔尾 SEGMENTS 與 repo-projects-check.md。
//
// ui_preview 的寫入端點只記錄請求（stdout 一行 `write-request <METHOD> <PATH> <BODY>`）、不改投影
// （repo-projects design Risks「ui_preview 沒有真的後端狀態」）。所以「新投影到達」一律由本腳本在頁面裡
// 呼叫 window.onState（channel.js 收到 /ws 訊息時呼叫的同一個入口）注入特製投影，做法同
// docs/research/2026-09-23/visual-check.js 的 injectState()：先把 window.onState 換成空函式擋掉之後真的
// /ws 推送，再以原本的 onState 畫出特製投影。
//
// 埠：preview 從 7950 起、CDP 從 19610 起（pickPort 遇到占用就往上找）。這兩段沒有被其他驗收腳本用過
// （2026-10-08 grep docs/research/*/*.js 的 pickPort／PORT 常數：preview 用過 7770、7780、7792、7793、7830、7870、
// 7910、7970、7990；CDP 用過 18781–18991、19000–19200、19310、19410–19440、19510、9333）。開跑前 7950 或 19610
// 已有人 LISTEN 就直接結束（exit 2），不搶、不動別人的行程。7770 有人 LISTEN 時只印一行「注意」（那是使用者的
// cockpit）；7778 是與本專案無關的 ASUS 服務，本腳本不碰。
//
// 整頁重畫的判斷：讀 `#version` 的 `data-state-version` 屬性（render.js 每次 paint() 寫入投影 version）。
// `#version` 的 textContent 是固定的程式版本號，不會隨投影改變，不能拿來判斷重畫。
'use strict';

const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

const PREVIEW_PORT_BASE = 7950;
const CDP_PORT_BASE = 19610;
// 推送間隔拉長，讓背景輪替在段落期間靜止；需要頻繁重畫的段落以 env 覆寫。
const STABLE_PUSH_MS = '600000';
const CHROME_UDD_PREFIX = 'cockpit-chrome-repoprojcheck-';
const PREVIEW_TEMP_PREFIX = 'cockpit-ui-preview-';
const UI_TIMEOUT_MS = 5000;

// ui_preview fixture（cockpit/examples/ui_preview.rs 的 PREVIEW_DETECTED_REPOS 與 Repo Project 情境）。
const BILLING = { repo: 'd:\\work\\billing-api\\.git', name: 'billing-api', paneCount: 2, id: 'billing-api' };
const DOCS = { repo: 'd:\\work\\docs-site\\.git', name: 'Docs Site', paneCount: 1, id: 'Docs-Site' };
const FIXTURE_PROJECTS = ['cockpit', 'p', 'demo-app'];
// spec cockpit-dashboard「Project 切換」：預設 stages 依介面語言。
const DEFAULT_STAGES = { zh: ['規劃', '實作', '審查', '完成'], en: ['Plan', 'Implement', 'Review', 'Complete'] };

// ---------------------------------------------------------------------------
// 命令列
// ---------------------------------------------------------------------------

const POSITIONAL = process.argv.slice(2);
const SEGMENT_ARG = POSITIONAL[0];

// 代號以逗號分隔；每個代號必須完全等於某段的代號，或是以 `/` 結尾的前綴（選該前綴的全部段落，至少要
// 選中一段）。任何一個不合格就整體拒絕。
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
// 前置條件：不成立就 FAIL 並結束這一段（後面的斷言建立在它之上，繼續跑只會產生一串連帶失敗）。
class SegmentAbort extends Error {}
function need(cond, label) {
  if (!check(cond, label)) throw new SegmentAbort(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const J = (v) => JSON.stringify(v);

// ---------------------------------------------------------------------------
// 行程管理（同 split-check.js；只終止本腳本自己 spawn、還握著 ChildProcess 且沒觀察到 exit 的行程）
// ---------------------------------------------------------------------------

function pidStillRunning(pid) {
  const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
  return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
}
function killTree(child, label) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
  check(!pidStillRunning(child.pid), `${label} PID ${child.pid} 已終止（tasklist 查無此 PID）`);
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
const OUR_PORTS = new Set(); // 本次執行用過的 preview／CDP 埠（收尾逐一確認沒有 LISTEN）
const OUR_PIDS = new Set(); // 本次執行 spawn 過的 PID（收尾逐一確認已不存在）
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
async function settleChild(child, port, label) {
  const exited = await waitForChildExit(child, 5000);
  const portListening = isPortListening(port);
  if (exited && !portListening) {
    const i = SPAWNED.findIndex((e) => e.child === child);
    if (i !== -1) SPAWNED.splice(i, 1);
  }
  check(exited, `${label} 應該觀察到子行程的 exit 事件（PID ${child.pid}）`);
  check(!portListening, `${label} 的 port ${port} 應該不再有 LISTENING 的行程`);
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
// CDP（同 split-check.js 的 CDP class）
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
  // 在頁面裡執行一個（不捕捉 Node 端變數的）函式；參數以 JSON 傳入。
  run(fn, ...args) {
    return this.eval(`(${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')})`);
  }
  // 輪詢頁面函式直到回傳 truthy；回傳最後一次的值（逾時為 false），不自己 check()。
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
  // 點擊 fn(...args) 回傳的元素：捲進可視範圍、以中心點命中測試確認點得到它（或其子孫），再送真的滑鼠事件。
  async clickEl(fn, args, desc) {
    const locate = `(() => { const el = (${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')});
      if (!el) return null; el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, w: r.width, h: r.height }; })()`;
    let pt = await this.eval(locate);
    if (!pt || pt.w === 0 || pt.h === 0) {
      check(false, `找不到可點的元素（或尺寸為 0）：${desc}`);
      return false;
    }
    for (let attempt = 1; ; attempt += 1) {
      await sleep(80);
      const hit = await this.eval(`(() => { const el = (${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')});
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
  // 真的鍵盤事件（頁面 keydown 監聽器看得到 event.key）。Enter：pressKey('Enter', 'Enter', 13, '\r')。
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
  if (chrome.pid) OUR_PIDS.add(chrome.pid);
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
  killTree(handle.chrome, label);
  await settleChild(handle.chrome, handle.cdpPort, label);
  await removeDirWithRetry(handle.udd);
}

async function removeDirWithRetry(dir) {
  for (let i = 0; i < 20; i++) {
    try {
      fs.rmSync(dir, { recursive: true, force: true });
    } catch {
      // Windows 偶爾 EBUSY／EPERM（檔案還被剛結束的行程鎖著），稍後重試。
    }
    if (!fs.existsSync(dir)) return true;
    await sleep(250);
  }
  return !fs.existsSync(dir);
}

// ui_preview 啟動時把 file-review 的假 repo 複製到 %TEMP% 下（stdout 印 review-repo／other-repo 路徑），
// 以 taskkill /F 結束時沒有機會自己清，由 stopPreview() 刪。寫入請求的記錄行（write-request）收進 writes。
const PREVIEW_TEMP_ROOTS = new Set();
async function startPreview(envOverrides, label) {
  if (!fs.existsSync(UI_PREVIEW_EXE)) {
    throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  }
  const port = pickPort(PREVIEW_PORT_BASE);
  const info = { reviewRepo: null, otherRepo: null };
  const writes = []; // { method, path, body, at }
  const server = spawn(UI_PREVIEW_EXE, [], {
    stdio: ['ignore', 'pipe', 'ignore'],
    windowsHide: true,
    env: { ...process.env, COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`, COCKPIT_PREVIEW_PUSH_MS: STABLE_PUSH_MS, ...envOverrides },
  });
  server.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
  SPAWNED.push({ child: server, label: `${label}（preview）`, port });
  OUR_PORTS.add(port);
  if (server.pid) OUR_PIDS.add(server.pid);
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
      if (m) writes.push({ method: m[1], path: m[2], body: m[3], at: Date.now() });
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
  killTree(preview.server, label);
  await settleChild(preview.server, preview.port, label);
  const root = preview.tempRoot;
  if (root && path.basename(root).startsWith(PREVIEW_TEMP_PREFIX)) {
    const removed = await removeDirWithRetry(root);
    check(removed, `${label} 的暫存副本目錄已刪除（${root}）`);
    if (removed) PREVIEW_TEMP_ROOTS.delete(root);
  }
}

// ---------------------------------------------------------------------------
// 頁面端工具：以 Page.addScriptToEvaluateOnNewDocument 安裝成 window.__rp（重新整理後仍在）。
// 定位規則（前端契約，repo-projects-check.md「前端契約」）：
//   C1 左欄 Project 分頁＝[data-region="projects"]；Project 項目＝[data-action="select-project"][data-project]。
//   C2 偵測到的 repo 區＝[data-region="projects"] 內的 .detected-repos；每項＝.detected-repo[data-repo]，名稱
//      .detected-repo-name、數量 .detected-repo-count、加入鈕 [data-action="add-repo"][data-repo]。
//   C3 Factory Floor 顯示的 Project＝[data-region="floor"] .project[data-project]；空狀態＝.floor-empty-state。
//   C4（task 5.2）「⋯」＝[data-region="projects"] [data-action="project-menu"][data-project]（<button>，aria-expanded、
//      aria-controls 指向選單 id）；選單＝.project-menu[data-project]，項目 [data-action="project-rename"|
//      "project-edit-stages"|"project-remove"][data-project]。
//   C5（task 5.2）對話框＝dialog.project-dialog（body 底下、#app 之外；開啟時有 open 屬性，data-dialog 為
//      rename／stages／remove）。標題 .project-dialog-title、說明 .project-dialog-text、改名輸入 input.project-dialog-name、
//      stage 列 .stage-row（輸入 input.stage-name，列內 [data-stage-op="up"|"down"|"delete"]）、新增
//      [data-stage-op="add"]、底部 [data-dialog-op="cancel"|"submit"]、錯誤 .project-dialog-error（hidden 以外即顯示）。
// ---------------------------------------------------------------------------

function pageHelpers() {
  if (window.__rp) return;
  const txt = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : '');
  const region = () => document.querySelector('[data-region="projects"]');
  const section = () => (region() ? region().querySelector('.detected-repos') : null);
  const items = () => (section() ? Array.from(section().querySelectorAll('.detected-repo')) : []);
  const addButtons = () => (region() ? Array.from(region().querySelectorAll('[data-action="add-repo"]')) : []);
  const addButton = (repo) => addButtons().find((b) => b.getAttribute('data-repo') === repo) || null;
  const projectItems = () => (region() ? Array.from(region().querySelectorAll('[data-action="select-project"]')) : []);
  const projectItem = (id) => projectItems().find((b) => b.getAttribute('data-project') === id) || null;
  const stateVersion = () => {
    const v = document.getElementById('version');
    return v && v.hasAttribute('data-state-version') ? v.getAttribute('data-state-version') : null;
  };
  // 左欄與 Factory Floor 的快照。
  const snapshot = () => {
    const sel = projectItems().find((b) => b.classList.contains('selected'));
    const shown = document.querySelector('[data-region="floor"] .project[data-project]');
    const floorEmpty = document.querySelector('[data-region="floor"] .floor-empty-state');
    const sec = section();
    return {
      projects: projectItems().map((b) => b.getAttribute('data-project')),
      selected: sel ? sel.getAttribute('data-project') : null,
      selectedCount: projectItems().filter((b) => b.classList.contains('selected')).length,
      shown: shown ? shown.getAttribute('data-project') : null,
      floorEmpty: floorEmpty ? txt(floorEmpty) : null,
      projectsEmpty: txt(region() ? region().querySelector('.projects-empty-state') : null) || null,
      section: !!sec,
      sectionInRegion: !!sec && !!region() && region().contains(sec),
      sectionAfterList: (() => {
        const list = region() ? region().querySelector('.project-list') : null;
        return !!sec && (!list || !!(list.compareDocumentPosition(sec) & Node.DOCUMENT_POSITION_FOLLOWING));
      })(),
      sectionTitle: txt(sec ? sec.querySelector('.detected-repos-title') : null),
      sectionEmpty: txt(sec ? sec.querySelector('.detected-repos-empty') : null) || null,
      repos: items().map((it) => {
        const btn = it.querySelector('[data-action="add-repo"]');
        return {
          repo: it.getAttribute('data-repo'),
          name: txt(it.querySelector('.detected-repo-name')),
          nameTitle: it.querySelector('.detected-repo-name') ? it.querySelector('.detected-repo-name').title : null,
          count: txt(it.querySelector('.detected-repo-count')),
          button: btn
            ? {
                text: txt(btn),
                repo: btn.getAttribute('data-repo'),
                tag: btn.tagName,
                label: btn.getAttribute('aria-label'),
                ariaDisabled: btn.getAttribute('aria-disabled'),
                disabledAttr: btn.hasAttribute('disabled'),
                busy: btn.getAttribute('aria-busy'),
              }
            : null,
        };
      }),
      addButtons: addButtons().length,
      error: txt(document.querySelector('.error-banner .action-banner-text')) || null,
      active: (() => {
        const a = document.activeElement;
        if (!a || a === document.body) return null;
        return { action: a.getAttribute('data-action'), repo: a.getAttribute('data-repo'), project: a.getAttribute('data-project'), focusVisible: a.matches(':focus-visible') };
      })(),
    };
  };
  // 注入特製投影（同 visual-check.js injectState）：擋掉之後真的 /ws 推送，以原本的 onState 畫出 state。
  const inject = (state) => {
    if (!window.__rpOrigOnState) window.__rpOrigOnState = window.onState;
    window.onState = function () {};
    window.__rpOrigOnState(state);
    return true;
  };
  const latest = () => (typeof window.cockpitLatestState === 'function' ? JSON.parse(JSON.stringify(window.cockpitLatestState())) : null);
  // task 5.2：管理選單與對話框（前端契約 C4、C5）。
  const byData = (nodes, attr, value) => Array.from(nodes).find((n) => n.getAttribute(attr) === value) || null;
  const menuButton = (pid) => byData(document.querySelectorAll('#app [data-region="projects"] [data-action="project-menu"]'), 'data-project', pid);
  const menu = (pid) => byData(document.querySelectorAll('#app [data-region="projects"] .project-menu'), 'data-project', pid);
  const menuItem = (pid, action) => {
    const m = menu(pid);
    return m ? byData(m.querySelectorAll('[data-action]'), 'data-action', action) : null;
  };
  const menuSnap = (pid) => {
    const b = menuButton(pid);
    const m = menu(pid);
    return {
      button: b
        ? { tag: b.tagName, text: txt(b), expanded: b.getAttribute('aria-expanded'), label: b.getAttribute('aria-label'), controls: b.getAttribute('aria-controls') }
        : null,
      open: !!m,
      menuId: m ? m.id : null,
      items: m ? Array.from(m.querySelectorAll('[data-action]')).map((n) => ({ action: n.getAttribute('data-action'), project: n.getAttribute('data-project'), text: txt(n), tag: n.tagName })) : [],
      menuButtons: document.querySelectorAll('#app [data-action="project-menu"]').length,
      menuButtonProjects: Array.from(document.querySelectorAll('#app [data-action="project-menu"]')).map((n) => n.getAttribute('data-project')),
    };
  };
  const dialog = () => document.querySelector('dialog.project-dialog');
  const stageRows = () => (dialog() ? Array.from(dialog().querySelectorAll('.stage-row')) : []);
  const stageInput = (i) => (stageRows()[i] ? stageRows()[i].querySelector('input.stage-name') : null);
  const nameInput = () => (dialog() ? dialog().querySelector('input.project-dialog-name') : null);
  // 對話框按鈕：row 為 null 時找底部（data-dialog-op）或「新增」（data-stage-op="add"）；否則找第 row 列的列內按鈕。
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
    const rowIndex = rows.findIndex((r) => r.contains(a));
    return {
      tag: a.tagName,
      cls: a.className || '',
      op: a.getAttribute('data-stage-op') || a.getAttribute('data-dialog-op'),
      action: a.getAttribute('data-action'),
      project: a.getAttribute('data-project'),
      row: rowIndex,
      value: typeof a.value === 'string' && a.tagName === 'INPUT' ? a.value : null,
      focusVisible: a.matches(':focus-visible'),
      invalid: a.getAttribute('aria-invalid'),
      describedby: a.getAttribute('aria-describedby'),
      describedText: a.getAttribute('aria-describedby') ? txt(document.getElementById(a.getAttribute('aria-describedby'))) : null,
      borderColor: getComputedStyle(a).borderTopColor,
    };
  };
  // task 6.1 F4：對話框內所有帶 aria-invalid／aria-describedby 的輸入框（錯誤清除後應為空）。
  const invalidInputs = () =>
    dialog() ? Array.from(dialog().querySelectorAll('input')).filter((i) => i.hasAttribute('aria-invalid') || i.hasAttribute('aria-describedby')).length : 0;
  const dialogSnap = () => {
    const d = dialog();
    if (!d) return null;
    const app = document.getElementById('app');
    const err = d.querySelector('.project-dialog-error');
    const a = document.activeElement;
    return {
      open: d.open,
      modal: d.matches(':modal'),
      kind: d.open ? d.getAttribute('data-dialog') : null,
      inApp: !!app && app.contains(d),
      title: txt(d.querySelector('.project-dialog-title')),
      text: txt(d.querySelector('.project-dialog-text')),
      name: nameInput() ? nameInput().value : null,
      stages: stageRows().map((r) => (r.querySelector('input.stage-name') || {}).value),
      error: err && !err.hidden ? txt(err) || null : null,
      activeInDialog: !!a && d.contains(a),
      active: describe(a),
      imgs: d.querySelectorAll('img').length,
    };
  };
  const activeDesc = () => describe(document.activeElement);
  // 以 CSS 變數（--accent、--bad…）算出的實際顏色字串，斷言不寫死 rgb 值（token 改了不必改腳本）。
  const cssColor = (name) => {
    const probe = document.createElement('span');
    probe.style.color = `var(--${name})`;
    document.body.appendChild(probe);
    const c = getComputedStyle(probe).color;
    probe.remove();
    return c;
  };
  window.__rp = {
    cssColor, txt, region, section, items, addButton, projectItem, stateVersion, snapshot, inject, latest,
    menuButton, menu, menuItem, menuSnap, dialog, stageRows, stageInput, nameInput, dialogButton, dialogSnap, activeDesc, invalidInputs,
  };
}

// ---------------------------------------------------------------------------
// 段落共用：啟動 preview＋chrome、導覽、等首份投影；收尾
// ---------------------------------------------------------------------------

// 「第一份真投影已經畫出」：頂列有 [data-runtime]，且 #version 的 data-state-version 等於 /api/state 的 version。
async function waitForFirstProjection(cdp, previewPort) {
  const start = Date.now();
  while (Date.now() - start < 8000) {
    const s = await cdp
      .eval(`(() => ({ lamp: !!document.querySelector('[data-region="topbar"] [data-runtime]'), v: window.__rp ? window.__rp.stateVersion() : null }))()`)
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

const stateVersion = (ctx) => ctx.cdp.run(() => window.__rp.stateVersion());
const snap = (ctx) => ctx.cdp.run(() => window.__rp.snapshot());

// 網路紀錄：只記 /api/repo-projects 的請求與回應（等「加入」的回應到達用）。
async function recordRepoProjectRequests(cdp) {
  const reqs = [];
  const byId = new Map();
  cdp.onEvent('Network.requestWillBeSent', (p, sid) => {
    if (sid || !/\/api\/repo-projects(\/|$|\?)/.test(p.request.url)) return;
    const r = { id: p.requestId, method: p.request.method, url: p.request.url, at: Date.now(), status: null, finishedAt: null };
    byId.set(p.requestId, r);
    reqs.push(r);
  });
  cdp.onEvent('Network.responseReceived', (p, sid) => {
    const r = sid ? null : byId.get(p.requestId);
    if (r) r.status = p.response.status;
  });
  cdp.onEvent('Network.loadingFinished', (p, sid) => {
    const r = sid ? null : byId.get(p.requestId);
    if (r) r.finishedAt = Date.now();
  });
  await cdp.send('Network.enable');
  return reqs;
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
    ctx.repoReqs = await recordRepoProjectRequests(ctx.cdp);
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

// --- 使用者操作（全部經由畫面：真的滑鼠／鍵盤事件）---

async function clickAdd(ctx, repo) {
  need(await ctx.cdp.clickEl((r) => window.__rp.addButton(r), [repo], `「加入」${repo}`), `點 ${repo} 的「加入」`);
}
async function clickProject(ctx, id) {
  need(await ctx.cdp.clickEl((p) => window.__rp.projectItem(p), [id], `左欄 Project ${id}`), `點左欄 Project ${id}`);
  const ok = await ctx.cdp.poll((p) => window.__rp.snapshot().selected === p, [id], UI_TIMEOUT_MS);
  need(!!ok, `左欄選定 ${id}`);
}
// 頂列的語言切換鈕（真的點擊；setLang 寫 cockpit.lang 後整頁重新載入）→ 等新語言的首份投影。
async function toggleLanguage(ctx, expectLang) {
  need(
    await ctx.cdp.clickEl(() => document.querySelector('[data-action="toggle-language"]'), [], '頂列語言切換鈕'),
    '點頂列語言切換鈕'
  );
  const switched = await ctx.cdp.poll((lg) => window.cockpitI18n && window.cockpitI18n.lang === lg && !!window.__rp, [expectLang], 8000);
  need(!!switched, `重新載入後介面語言為 ${expectLang}`);
  await waitForFirstProjection(ctx.cdp, ctx.preview.port);
}
// 等服務收到第 n 筆（從 1 起算）POST /api/repo-projects；回傳該筆的本體（已 JSON.parse；失敗為 { raw }）。
async function waitAddRequest(ctx, n, timeoutMs = UI_TIMEOUT_MS) {
  const start = Date.now();
  for (;;) {
    const posts = ctx.preview.writes.filter((w) => w.method === 'POST' && w.path === '/api/repo-projects');
    if (posts.length >= n) {
      const w = posts[n - 1];
      try {
        return { parsed: JSON.parse(w.body), raw: w.body };
      } catch {
        return { parsed: null, raw: w.body };
      }
    }
    if (Date.now() - start >= timeoutMs) return null;
    await sleep(50);
  }
}
const addPosts = (ctx) => ctx.preview.writes.filter((w) => w.method === 'POST' && w.path === '/api/repo-projects');
// 等頁面收到第 n 筆 POST /api/repo-projects 的回應（loadingFinished）。
async function waitAddResponse(ctx, n, timeoutMs = UI_TIMEOUT_MS) {
  const start = Date.now();
  for (;;) {
    const posts = ctx.repoReqs.filter((r) => r.method === 'POST');
    if (posts.length >= n && posts[n - 1].finishedAt !== null) return posts[n - 1];
    if (Date.now() - start >= timeoutMs) return null;
    await sleep(50);
  }
}

// 特製投影：以目前畫面上的最新投影為底，version 遞增；`addProject` 把偵測區的某個 repo「加入」成 Repo Project
// （從 detected_repos 拿掉、在 projects 最後加上一個 kind repo 的 Project）。工作線與 task 留空（fix round 1 審查
// Minor 4）：真後端一個 pane 只屬於一個 repo，不能複製 demo-app 綁 wJ:p6／wJ:p7 的工作線。
async function craft(ctx, mutate) {
  const base = await ctx.cdp.run(() => window.__rp.latest());
  need(!!base && Array.isArray(base.projects), '取得頁面目前的投影（window.cockpitLatestState）');
  const s = JSON.parse(JSON.stringify(base));
  s.version = Number(base.version) + 1;
  mutate(s);
  return s;
}
function addProject(s, detected, stages) {
  const demo = s.projects.find((p) => p.id === 'demo-app');
  const p = JSON.parse(JSON.stringify(demo));
  p.id = detected.id;
  p.name = detected.name;
  p.repo = detected.repo;
  p.kind = 'repo';
  p.stages = stages.slice();
  p.warnings = [];
  p.warning_msgs = [];
  p.workstreams = [];
  p.tasks = [];
  s.projects.push(p);
  s.detected_repos = s.detected_repos.filter((d) => d.repo !== detected.repo);
}
// 與 addProject 相反：billing-api 的 Project 被移除、repo 回到偵測區最前面。
function backToDetected(s) {
  s.projects = s.projects.filter((p) => p.id !== BILLING.id);
  s.detected_repos = s.detected_repos.filter((d) => d.repo !== BILLING.repo);
  s.detected_repos.unshift({ repo: BILLING.repo, name: BILLING.name, pane_count: BILLING.paneCount });
}
async function inject(ctx, state, label) {
  const from = await stateVersion(ctx);
  need(await ctx.cdp.run((st) => window.__rp.inject(st), state), `注入特製投影：${label}`);
  const ok = await ctx.cdp.poll((v) => window.__rp.stateVersion() === v, [String(state.version)], UI_TIMEOUT_MS);
  need(!!ok, `特製投影（${label}）已重畫（data-state-version ${from} → ${state.version}）`);
}

// 檢查左欄的偵測區列出 expected（依序；名稱、數量、加入鈕）。
function checkDetectedList(s, expected, lang, label) {
  check(s.section && s.sectionInRegion, `${label}：左欄 Project 分頁（[data-region="projects"]）內有偵測到的 repo 區`);
  check(s.sectionAfterList, `${label}：偵測到的 repo 區在 Project 清單之後`);
  const want = lang === 'zh' ? '偵測到的 repo' : 'Detected repos';
  check(s.sectionTitle === want, `${label}：偵測區標題為 ${J(want)}（實際 ${J(s.sectionTitle)}）`);
  check(
    J(s.repos.map((r) => r.repo)) === J(expected.map((e) => e.repo)),
    `${label}：偵測區依投影順序列出 ${J(expected.map((e) => e.name))}（實際 ${J(s.repos.map((r) => r.name))}）`
  );
  for (const e of expected) {
    const r = s.repos.find((x) => x.repo === e.repo);
    if (!r) continue;
    check(r.name === e.name, `${label}：${e.name} 的名稱文字為 ${J(e.name)}（實際 ${J(r.name)}）`);
    const countWant = lang === 'zh' ? `${e.paneCount} 個 pane` : `${e.paneCount} ${e.paneCount === 1 ? 'pane' : 'panes'}`;
    check(r.count === countWant, `${label}：${e.name} 顯示 pane 數 ${J(countWant)}（實際 ${J(r.count)}）`);
    const btnWant = lang === 'zh' ? '加入' : 'Add';
    check(
      !!r.button && r.button.tag === 'BUTTON' && r.button.text === btnWant && r.button.repo === e.repo,
      `${label}：${e.name} 有「${btnWant}」鈕（<button>，data-repo 為 repo key；實際 ${J(r.button)}）`
    );
  }
  check(s.addButtons === expected.length, `${label}：左欄的「加入」鈕恰有 ${expected.length} 個（實際 ${s.addButtons}）`);
}

// ---------------------------------------------------------------------------
// 段落：自我測試
// ---------------------------------------------------------------------------

// parseSegmentArg 的合法與不合法輸入；再實際以拼錯的代號與空字串執行本檔：必須 exit 2、印
// `RESULT: FAIL (段落代號)`，而且不啟動任何行程（參數檢查在環境檢查與啟動之前）。
async function segSelfSegmentArg() {
  const known = ['self/a', 'self/b', 'cockpit-dashboard/x y'];
  const cases = [
    [undefined, true, null],
    ['self/a', true, ['self/a']],
    ['self/', true, ['self/a', 'self/b']],
    ['self/a, cockpit-dashboard/x y', true, ['self/a', 'cockpit-dashboard/x y']],
    ['self/a,self/a', true, ['self/a']],
    ['Self/a', false, null],
    ['nope/', false, null],
    ['', false, null],
    [',,', false, null],
  ];
  for (const [arg, ok, codes] of cases) {
    const r = parseSegmentArg(arg, known);
    const pass = r.ok === ok && (!ok || JSON.stringify(r.codes) === JSON.stringify(codes));
    check(pass, `parseSegmentArg(${JSON.stringify(arg)}) → ok=${ok}${ok ? ` codes=${JSON.stringify(codes)}` : ''}（實際 ${JSON.stringify(r)}）`);
  }
  for (const bad of ['不存在的段落', '']) {
    const r = spawnSync(process.execPath, [__filename, bad], { encoding: 'utf8', timeout: 30000 });
    check(r.status === 2, `以 ${JSON.stringify(bad)} 執行本檔 exit 2（實際 ${r.status}）`);
    check(/RESULT: FAIL \(段落代號\)/.test(r.stdout || ''), `以 ${JSON.stringify(bad)} 執行本檔印出 RESULT: FAIL (段落代號)`);
    check(!/===/.test(r.stdout || ''), `以 ${JSON.stringify(bad)} 執行本檔沒有進入任何段落`);
  }
}

// ---------------------------------------------------------------------------
// 段落：cockpit-dashboard「Project 切換」（repo-projects task 5.1）
// ---------------------------------------------------------------------------

// scenario「列出偵測到的 repo」：fixture 的 detected_repos 為 billing-api（2）與 Docs Site（1），projects 有
// cockpit、p、demo-app。左欄在 Project 清單之外有偵測區，依序列出兩者、各有「加入」鈕；英文介面同樣列出。
async function segListDetected() {
  await withCockpit('list', {}, async (ctx) => {
    let s = await snap(ctx);
    check(J(s.projects) === J(FIXTURE_PROJECTS), `Project 清單仍是 ${J(FIXTURE_PROJECTS)}（實際 ${J(s.projects)}）`);
    checkDetectedList(s, [BILLING, DOCS], 'zh', '繁中');
    check(s.repos.every((r) => r.nameTitle === r.name), `名稱節點的 title 是完整名稱（長名稱截斷時看得到；實際 ${J(s.repos.map((r) => r.nameTitle))}）`);
    check(
      s.repos.every((r) => r.button && r.button.label === `加入 ${r.name}`),
      `「加入」鈕的無障礙名稱含 repo 名稱（「加入 <名稱>」；實際 ${J(s.repos.map((r) => r.button && r.button.label))}）`
    );
    check(addPosts(ctx).length === 0, '只是重畫，服務沒有收到任何 POST /api/repo-projects');
    // task 6.1 F6：pane 數用 sans＋tabular-nums（數字等寬、中文字不被等寬字體撐開），與 .project-count 一致。
    const fonts = await ctx.cdp.run(() => {
      const c = getComputedStyle(document.querySelector('.detected-repo-count'));
      const p = document.querySelector('.project-count');
      return { family: c.fontFamily, numeric: c.fontVariantNumeric, projectFamily: p ? getComputedStyle(p).fontFamily : null };
    });
    check(
      !!fonts.projectFamily && fonts.family === fonts.projectFamily && !/mono/i.test(fonts.family) && /tabular-nums/.test(fonts.numeric),
      `pane 數與 .project-count 同為 sans 且 font-variant-numeric 含 tabular-nums（實際 ${J(fonts)}）`
    );
    // task 6.1 F2：偵測區標題縮到文字寬度，焦點外框不橫跨整欄。
    const tw = await ctx.cdp.run(() => {
      const t = document.querySelector('.detected-repos-title');
      return { title: t.getBoundingClientRect().width, section: t.closest('.detected-repos').getBoundingClientRect().width, radius: getComputedStyle(t).borderTopLeftRadius };
    });
    check(tw.title > 0 && tw.title < tw.section - 40, `偵測區標題寬度為文字寬度、不撐滿整欄（標題 ${tw.title}px、區塊 ${tw.section}px）`);
    await toggleLanguage(ctx, 'en');
    s = await snap(ctx);
    checkDetectedList(s, [BILLING, DOCS], 'en', '英文');
    check(
      s.repos.every((r) => r.button && r.button.label === `Add ${r.name}`),
      `英文「Add」鈕的無障礙名稱含 repo 名稱（實際 ${J(s.repos.map((r) => r.button && r.button.label))}）`
    );
    noExceptions(ctx);
  });
}

// scenario「按加入送出預設 stages」：繁中介面以滑鼠按 billing-api 的「加入」→ 服務收到 POST /api/repo-projects，
// 本體恰為 {"repo":…,"stages":["規劃","實作","審查","完成"]}（沒有 name）；以鍵盤（焦點在 Docs Site 的「加入」上按 Enter）
// 同樣送出。切換成英文後再按一次，stages 為 Plan、Implement、Review、Complete。
async function segAddSendsDefaultStages() {
  await withCockpit('add', {}, async (ctx) => {
    await clickAdd(ctx, BILLING.repo);
    const first = await waitAddRequest(ctx, 1);
    need(first !== null, '按「加入」後服務收到 POST /api/repo-projects');
    const wantZh = { repo: BILLING.repo, stages: DEFAULT_STAGES.zh };
    check(J(first.parsed) === J(wantZh), `繁中：本體恰為 ${J(wantZh)}（沒有 name；實際 ${first.raw}）`);
    check(first.parsed && !('name' in first.parsed), '繁中：本體沒有 name 欄位');
    const r1 = await waitAddResponse(ctx, 1);
    check(r1 !== null && r1.status === 201, `假端點以正式端點的規則驗證本體並回 201（實際 ${r1 ? r1.status : '沒有回應'}）`);
    let s = await snap(ctx);
    check(s.error === null, `成功的加入不顯示錯誤訊息（實際 ${J(s.error)}）`);
    check(
      s.active && s.active.action === 'add-repo' && s.active.repo === BILLING.repo && s.active.focusVisible === false,
      `滑鼠按下後焦點在被按的「加入」鈕上、不呈現焦點外框（實際 ${J(s.active)}）`
    );

    // 鍵盤：焦點移到 Docs Site 的「加入」→ Enter。
    need(await ctx.cdp.run((r) => { const b = window.__rp.addButton(r); if (!b) return false; b.focus(); return document.activeElement === b; }, DOCS.repo), '焦點移到 Docs Site 的「加入」');
    await ctx.cdp.pressKey('Enter', 'Enter', 13, '\r');
    const second = await waitAddRequest(ctx, 2);
    need(second !== null, '在「加入」上按 Enter 後服務收到第二筆 POST /api/repo-projects');
    const wantZh2 = { repo: DOCS.repo, stages: DEFAULT_STAGES.zh };
    check(J(second.parsed) === J(wantZh2), `鍵盤：本體恰為 ${J(wantZh2)}（實際 ${second.raw}）`);
    await sleep(300);
    check(addPosts(ctx).length === 2, `一次按壓只送一筆（共 2 筆；實際 ${addPosts(ctx).length}）`);

    // 切換成英文（頂列切換鈕，整頁重新載入）後再按一次。
    await toggleLanguage(ctx, 'en');
    await clickAdd(ctx, BILLING.repo);
    const third = await waitAddRequest(ctx, 3);
    need(third !== null, '英文介面按「Add」後服務收到 POST /api/repo-projects');
    const wantEn = { repo: BILLING.repo, stages: DEFAULT_STAGES.en };
    check(J(third.parsed) === J(wantEn), `英文：本體恰為 ${J(wantEn)}（實際 ${third.raw}）`);
    const r3 = await waitAddResponse(ctx, 3);
    check(r3 !== null && r3.status === 201, `英文：假端點回 201（實際 ${r3 ? r3.status : '沒有回應'}）`);
    s = await snap(ctx);
    check(s.error === null, `英文：成功的加入不顯示錯誤訊息（實際 ${J(s.error)}）`);
    noExceptions(ctx);
  });
}

// scenario「加入成功後自動選定新 Project」：先選定 p；按 billing-api 的「加入」、等 201 {"id":"billing-api"} 到達；
// 在新投影到達之前選取不變；注入含 billing-api 的新投影（billing-api 從偵測區消失、出現在 Project 清單）後，左欄
// 選定 billing-api、Factory Floor 顯示它；之後點回 p，再來一份新投影也不會被自動切走（只選一次）。
async function segAutoSelectAfterAdd() {
  await withCockpit('autoselect', {}, async (ctx) => {
    await clickProject(ctx, 'p');
    await clickAdd(ctx, BILLING.repo);
    need((await waitAddRequest(ctx, 1)) !== null, '服務收到 POST /api/repo-projects');
    const r = await waitAddResponse(ctx, 1);
    need(r !== null && r.status === 201, `頁面收到 201 回應（實際 ${r ? r.status : '沒有回應'}）`);
    await sleep(300);
    let s = await snap(ctx);
    check(s.selected === 'p' && s.shown === 'p', `含新 id 的投影到達之前，選取不變（仍是 p；實際 selected=${s.selected} shown=${s.shown}）`);
    // fix round 1（審查 Important 1）：201 之後、偵測區仍列著這個 repo 時（真後端的投影還沒合併），「加入」維持停用，
    // 再按不送第二筆（否則第二筆會被 409 repo_already_added 拒絕、與成功並存）。
    const busy = s.repos.find((x) => x.repo === BILLING.repo);
    check(
      !!busy && !!busy.button && busy.button.ariaDisabled === 'true' && busy.button.disabledAttr === false,
      `201 後、偵測區仍列著 billing-api 時「加入」為 aria-disabled="true"（不用 disabled 屬性；實際 ${J(busy && busy.button)}）`
    );
    await clickAdd(ctx, BILLING.repo);
    await sleep(400);
    check(addPosts(ctx).length === 1, `停用中的「加入」再按不送出（共 1 筆；實際 ${addPosts(ctx).length}）`);
    check((await snap(ctx)).error === null, '停用中的「加入」再按不顯示錯誤');

    // 另一份不含新 id 的投影（例如期間的一般推送）不觸發選取。
    await inject(ctx, await craft(ctx, () => {}), '不含 billing-api 的一般投影');
    s = await snap(ctx);
    check(s.selected === 'p' && s.shown === 'p', `不含新 id 的投影到達時選取不變（實際 selected=${s.selected} shown=${s.shown}）`);

    await inject(ctx, await craft(ctx, (st) => addProject(st, BILLING, DEFAULT_STAGES.zh)), '含 billing-api 的新投影');
    s = await snap(ctx);
    check(J(s.projects) === J([...FIXTURE_PROJECTS, BILLING.id]), `billing-api 出現在 Project 清單（實際 ${J(s.projects)}）`);
    check(J(s.repos.map((x) => x.repo)) === J([DOCS.repo]), `billing-api 從偵測區消失（實際 ${J(s.repos.map((x) => x.name))}）`);
    check(s.selected === BILLING.id && s.selectedCount === 1, `新投影到達後左欄選定 billing-api（實際 selected=${s.selected}，選定項目 ${s.selectedCount} 個）`);
    check(s.shown === BILLING.id, `Factory Floor 顯示 billing-api（實際 ${s.shown}）`);

    await clickProject(ctx, 'p');
    await inject(ctx, await craft(ctx, () => {}), '之後又一份含 billing-api 的投影');
    s = await snap(ctx);
    check(s.selected === 'p' && s.shown === 'p', `點回 p 之後，下一份投影不會再自動切走（實際 selected=${s.selected} shown=${s.shown}）`);
    noExceptions(ctx);
  });
}

// 加入的回應晚於含新 id 的投影到達（服務先推送、HTTP 回應後到）：回應到達時就選定新 Project。
// 以 COCKPIT_PREVIEW_WRITE_RULES 讓 POST 延遲 1500 ms 才回 201。
async function segAutoSelectWhenResponseIsLate() {
  await withCockpit('late', { env: { COCKPIT_PREVIEW_WRITE_RULES: '/api/repo-projects=1500:201' } }, async (ctx) => {
    await clickAdd(ctx, BILLING.repo);
    need((await waitAddRequest(ctx, 1)) !== null, '服務收到 POST /api/repo-projects（回應延遲 1500 ms）');
    await inject(ctx, await craft(ctx, (st) => addProject(st, BILLING, DEFAULT_STAGES.zh)), '回應到達前就含 billing-api 的投影');
    let s = await snap(ctx);
    check(s.selected === 'cockpit', `回應到達之前不選定（仍是預設的第一個 cockpit；實際 ${s.selected}）`);
    const r = await waitAddResponse(ctx, 1, 4000);
    need(r !== null && r.status === 201, `頁面收到延遲的 201 回應（實際 ${r ? r.status : '沒有回應'}）`);
    const ok = await ctx.cdp.poll((id) => window.__rp.snapshot().selected === id, [BILLING.id], UI_TIMEOUT_MS);
    s = await snap(ctx);
    check(!!ok && s.shown === BILLING.id, `回應到達後選定 billing-api、Factory Floor 顯示它（實際 selected=${s.selected} shown=${s.shown}）`);
    noExceptions(ctx);
  });
}

// 加入被拒絕（409）時顯示錯誤訊息，且之後含該 repo 的投影到達也不自動選取。
async function segNoAutoSelectWhenRejected() {
  await withCockpit('rejected', { env: { COCKPIT_PREVIEW_WRITE_RULES: '/api/repo-projects=0:409' } }, async (ctx) => {
    await clickAdd(ctx, BILLING.repo);
    need((await waitAddRequest(ctx, 1)) !== null, '服務收到 POST /api/repo-projects（回 409）');
    const ok = await ctx.cdp.poll(() => window.__rp.snapshot().error !== null, [], UI_TIMEOUT_MS);
    let s = await snap(ctx);
    check(!!ok && /409/.test(s.error) && /ui_preview 模擬回應 409/.test(s.error), `頁面顯示含原因的錯誤訊息（實際 ${J(s.error)}）`);
    await inject(ctx, await craft(ctx, (st) => addProject(st, BILLING, DEFAULT_STAGES.zh)), '含 billing-api 的投影');
    s = await snap(ctx);
    check(s.selected === 'cockpit' && s.shown === 'cockpit', `加入失敗時不自動選定（實際 selected=${s.selected} shown=${s.shown}）`);
    check(s.error !== null, '之後的重畫不清除錯誤訊息');
    // fix round 1（審查 Important 1）：失敗後「加入」恢復可按。先讓 billing-api 回到偵測區（上面注入的投影拿掉了它）。
    await inject(ctx, await craft(ctx, backToDetected), 'billing-api 回到偵測區');
    s = await snap(ctx);
    const again = s.repos.find((x) => x.repo === BILLING.repo);
    check(!!again && !!again.button && again.button.ariaDisabled !== 'true', `409 之後 billing-api 的「加入」恢復可按（實際 ${J(again && again.button)}）`);
    check(!!again && !!again.button && again.button.text === '加入' && again.button.label === `加入 ${BILLING.name}` && again.button.busy === null, `409 之後文字與 aria-label 回到「加入」「加入 ${BILLING.name}」、aria-busy 移除（實際 ${J(again && again.button)}）`);
    await clickAdd(ctx, BILLING.repo);
    check((await waitAddRequest(ctx, 2)) !== null, '409 之後再按「加入」會送出第二筆 POST');
    noExceptions(ctx);
  });
  // 英文介面：409 失敗後文字與 aria-label 回到「Add」「Add <名稱>」。
  await withCockpit('rejected-en', { env: { COCKPIT_PREVIEW_WRITE_RULES: '/api/repo-projects=0:409' } }, async (ctx) => {
    await toggleLanguage(ctx, 'en');
    await clickAdd(ctx, BILLING.repo);
    need((await waitAddRequest(ctx, 1)) !== null, '英文：服務收到 POST /api/repo-projects（回 409）');
    need(!!(await ctx.cdp.poll(() => window.__rp.snapshot().error !== null, [], UI_TIMEOUT_MS)), '英文：頁面顯示錯誤訊息');
    await inject(ctx, await craft(ctx, () => {}), '失敗後的一般投影');
    const again = (await snap(ctx)).repos.find((x) => x.repo === BILLING.repo);
    check(!!again && !!again.button && again.button.text === 'Add' && again.button.label === `Add ${BILLING.name}` && again.button.busy === null && again.button.ariaDisabled !== 'true', `英文：409 之後文字與 aria-label 回到「Add」「Add ${BILLING.name}」（實際 ${J(again && again.button)}）`);
    noExceptions(ctx);
  });
}

// 頻繁重畫（每 100 ms 一份新投影）時焦點留在「加入」鈕上，按 Enter 照常送出；含新 id 的投影到達、「加入」鈕
// 隨 repo 離開偵測區而消失時，鍵盤焦點移到新選定的 Project 項目上（不掉回 <body>）。
async function segKeyboardFocusAcrossRepaints() {
  await withCockpit('focus', { env: { COCKPIT_PREVIEW_PUSH_MS: '100' } }, async (ctx) => {
    // 先以 Tab 讓焦點進入頁面（鍵盤輸入），再移到「加入」鈕上。
    await ctx.cdp.pressKey('Tab', 'Tab', 9, '');
    need(await ctx.cdp.run((r) => { const b = window.__rp.addButton(r); if (!b) return false; b.focus(); return document.activeElement === b; }, BILLING.repo), '焦點移到 billing-api 的「加入」');
    const v0 = await stateVersion(ctx);
    let lost = 0;
    for (let i = 0; i < 20; i += 1) {
      await sleep(100);
      const a = (await snap(ctx)).active;
      if (!(a && a.action === 'add-repo' && a.repo === BILLING.repo)) lost += 1;
    }
    const v1 = await stateVersion(ctx);
    check(Number(v1) - Number(v0) >= 10, `2 秒內至少重畫 10 次（data-state-version ${v0} → ${v1}）`);
    check(lost === 0, `重畫期間焦點始終在 billing-api 的「加入」上（20 次取樣有 ${lost} 次不在）`);
    await ctx.cdp.pressKey('Enter', 'Enter', 13, '\r');
    const req = await waitAddRequest(ctx, 1);
    need(req !== null, '頻繁重畫時按 Enter 照常送出 POST /api/repo-projects');
    check(J(req.parsed) === J({ repo: BILLING.repo, stages: DEFAULT_STAGES.zh }), `本體正確（實際 ${req.raw}）`);
    const r = await waitAddResponse(ctx, 1);
    need(r !== null && r.status === 201, '頁面收到 201 回應');
    await inject(ctx, await craft(ctx, (st) => addProject(st, BILLING, DEFAULT_STAGES.zh)), '含 billing-api 的新投影');
    const s = await snap(ctx);
    check(s.selected === BILLING.id, `選定 billing-api（實際 ${s.selected}）`);
    check(
      s.active && s.active.action === 'select-project' && s.active.project === BILLING.id,
      `「加入」鈕消失後鍵盤焦點移到新選定的 billing-api 項目上（實際 ${J(s.active)}）`
    );
    noExceptions(ctx);
  });
}

// scenario「空狀態指向偵測到的 repo」：projects 為空、detected_repos 有 app。Factory Floor 的空狀態文字指向
// 「偵測到的 repo」區、不含「重啟」（英文不含 Restart）；左欄「偵測到的 repo」區列出 app 與「加入」鈕。繁中與英文各一次。
async function segEmptyStatePointsToDetected() {
  await withCockpit('empty', {}, async (ctx) => {
    const APP = { repo: 'd:\\work\\app\\.git', name: 'app', paneCount: 2 };
    const emptyState = (st) => {
      st.projects = [];
      st.detected_repos = [{ repo: APP.repo, name: APP.name, pane_count: APP.paneCount }];
    };
    for (const lang of ['zh', 'en']) {
      if (lang === 'en') await toggleLanguage(ctx, 'en');
      await inject(ctx, await craft(ctx, emptyState), `${lang}：projects 為空、detected_repos 有 app`);
      const s = await snap(ctx);
      need(s.floorEmpty !== null, `${lang}：Factory Floor 顯示空狀態`);
      check(s.projects.length === 0, `${lang}：左欄沒有任何 Project 項目（實際 ${J(s.projects)}）`);
      if (lang === 'zh') {
        check(!/重啟/.test(s.floorEmpty), `繁中：空狀態文字不含「重啟」（實際 ${J(s.floorEmpty)}）`);
        check(/偵測到的 repo/.test(s.floorEmpty), `繁中：空狀態文字指向「偵測到的 repo」區（實際 ${J(s.floorEmpty)}）`);
        check(s.projectsEmpty === '沒有 Project', `繁中：左欄 Project 清單的空狀態仍是「沒有 Project」（實際 ${J(s.projectsEmpty)}）`);
      } else {
        check(!/restart/i.test(s.floorEmpty), `英文：空狀態文字不含 Restart（實際 ${J(s.floorEmpty)}）`);
        check(/Detected repos/.test(s.floorEmpty), `英文：空狀態文字指向 Detected repos 區（實際 ${J(s.floorEmpty)}）`);
        check(s.projectsEmpty === 'No Projects', `英文：左欄 Project 清單的空狀態仍是 No Projects（實際 ${J(s.projectsEmpty)}）`);
      }
      checkDetectedList(s, [APP], lang, `${lang}：空狀態`);
      check(s.shown === null, `${lang}：Factory Floor 沒有畫任何 Project`);
    }
    noExceptions(ctx);
  });
}

// 沒有偵測到的 repo：偵測區不列任何項目、沒有「加入」鈕，只有標題與說明（說明不是空白）。
async function segNoDetectedRepos() {
  await withCockpit('none', {}, async (ctx) => {
    await inject(ctx, await craft(ctx, (st) => { st.detected_repos = []; }), 'detected_repos 為空');
    const s = await snap(ctx);
    check(s.section && s.sectionInRegion, '偵測區仍在（空狀態的文字會指向它）');
    check(s.repos.length === 0 && s.addButtons === 0, `沒有任何項目與「加入」鈕（實際 ${s.repos.length} 項、${s.addButtons} 個鈕）`);
    check(typeof s.sectionEmpty === 'string' && s.sectionEmpty.length > 0, `有一行說明（實際 ${J(s.sectionEmpty)}）`);
    check(J(s.projects) === J(FIXTURE_PROJECTS), `Project 清單不受影響（實際 ${J(s.projects)}）`);
    noExceptions(ctx);
  });
}

// scenario「名稱不以 HTML 解讀」（偵測區部分）：repo 名稱含 HTML 時以原樣文字顯示、不建立 <img>、不執行指令碼；
// 按「加入」送出的 repo 仍是該項的 repo key。
async function segDetectedNameNotHtml() {
  await withCockpit('html', {}, async (ctx) => {
    const EVIL = { repo: 'd:\\work\\evil\\.git', name: '<img src=x onerror=window.__rpPwned=1>', paneCount: 1 };
    await inject(ctx, await craft(ctx, (st) => {
      st.detected_repos = [{ repo: EVIL.repo, name: EVIL.name, pane_count: EVIL.paneCount }];
    }), '名稱含 HTML 的偵測項目');
    await sleep(300);
    const s = await snap(ctx);
    checkDetectedList(s, [EVIL], 'zh', 'HTML 名稱');
    const dom = await ctx.cdp.run(() => ({ imgs: document.querySelectorAll('[data-region="projects"] img').length, pwned: window.__rpPwned === 1 }));
    check(dom.imgs === 0 && dom.pwned === false, `左欄沒有建立任何 <img>、沒有執行指令碼（實際 ${J(dom)}）`);
    await clickAdd(ctx, EVIL.repo);
    const req = await waitAddRequest(ctx, 1);
    need(req !== null, '服務收到 POST /api/repo-projects');
    check(req.parsed && req.parsed.repo === EVIL.repo, `送出的 repo 是該項的 repo key（實際 ${req.raw}）`);
    noExceptions(ctx);
  });
}

// 在元素中心送一次真的雙擊（mousePressed／mouseReleased 各兩次，clickCount 1 → 2），不做命中重試。
async function doubleClickAt(ctx, fn, args, desc) {
  const pt = await ctx.cdp.eval(`(() => { const el = (${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')});
    if (!el) return null; el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; })()`);
  need(!!pt, `找得到要雙擊的元素：${desc}`);
  await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: pt.x, y: pt.y });
  for (const clickCount of [1, 2]) {
    await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: pt.x, y: pt.y, button: 'left', clickCount });
    await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: pt.x, y: pt.y, button: 'left', clickCount });
  }
}

// fix round 1（審查 Important 1）：連點兩下「加入」只送一筆、不出現錯誤橫幅。第一下的同步重畫會在原位重建按鈕，
// 第二下仍落在「加入」上；送出中的狀態存在 actions.js（不在 DOM），第二下直接略過。
async function segDoubleClickSendsOnce() {
  await withCockpit('dblclick', {}, async (ctx) => {
    await doubleClickAt(ctx, (r) => window.__rp.addButton(r), [BILLING.repo], 'billing-api 的「加入」');
    need((await waitAddRequest(ctx, 1)) !== null, '雙擊後服務收到 POST /api/repo-projects');
    const r = await waitAddResponse(ctx, 1);
    check(r !== null && r.status === 201, `第一筆回 201（實際 ${r ? r.status : '沒有回應'}）`);
    await sleep(800);
    const s = await snap(ctx);
    check(addPosts(ctx).length === 1, `雙擊只送一筆 POST（實際 ${addPosts(ctx).length} 筆）`);
    check(s.error === null, `雙擊後沒有錯誤橫幅（實際 ${J(s.error)}）`);
    noExceptions(ctx);
  });
}

// fix round 1（審查 Important 1）：回應延遲 1500 ms、每 100 ms 重畫時，進行中的「加入」跨重畫維持 aria-disabled，
// 焦點仍在它上面時按 Enter 不送出；含新 Project 的投影讓 repo 離開偵測區後解除，之後 repo 再出現時可再按。
async function segAddInFlightAcrossRepaints() {
  await withCockpit('inflight', { env: { COCKPIT_PREVIEW_WRITE_RULES: '/api/repo-projects=1500:201', COCKPIT_PREVIEW_PUSH_MS: '100' } }, async (ctx) => {
    await clickAdd(ctx, BILLING.repo);
    need((await waitAddRequest(ctx, 1)) !== null, '服務收到 POST /api/repo-projects（回應延遲 1500 ms）');
    const v0 = await stateVersion(ctx);
    let notBusy = 0;
    for (let i = 0; i < 8; i += 1) {
      await sleep(100);
      const b = (await snap(ctx)).repos.find((x) => x.repo === BILLING.repo);
      if (!(b && b.button && b.button.ariaDisabled === 'true' && b.button.disabledAttr === false)) notBusy += 1;
    }
    const v1 = await stateVersion(ctx);
    check(Number(v1) - Number(v0) >= 4, `取樣期間至少重畫 4 次（data-state-version ${v0} → ${v1}）`);
    check(notBusy === 0, `進行中「加入」跨重畫維持 aria-disabled="true"、沒有 disabled 屬性（8 次取樣有 ${notBusy} 次不是）`);
    // task 6.1 F1：進行中可見文字與 aria-label 改成進行式，aria-busy 仍在；欄寬不跳動（與閒置的「加入」同寬）。
    const busyNow = await ctx.cdp.run((r1, r2) => {
      const b = window.__rp.addButton(r1);
      const idle = window.__rp.addButton(r2);
      return { text: window.__rp.txt(b), label: b.getAttribute('aria-label'), busy: b.getAttribute('aria-busy'), width: b.getBoundingClientRect().width, idleText: window.__rp.txt(idle), idleBusy: idle.getAttribute('aria-busy'), idleWidth: idle.getBoundingClientRect().width };
    }, BILLING.repo, DOCS.repo);
    check(busyNow.text === '加入中…' && busyNow.label === `加入中… ${BILLING.name}` && busyNow.busy === 'true', `繁中：進行中文字為「加入中…」、aria-label 為「加入中… ${BILLING.name}」、aria-busy="true"（實際 ${J(busyNow)}）`);
    check(busyNow.idleText === '加入' && busyNow.idleBusy === null, `繁中：沒在進行的「加入」不受影響（實際 ${J(busyNow)}）`);
    check(Math.abs(busyNow.width - busyNow.idleWidth) < 0.5, `繁中：進行中與閒置的按鈕同寬（欄寬不跳動；${busyNow.width} vs ${busyNow.idleWidth}）`);
    const a = (await snap(ctx)).active;
    check(!!a && a.action === 'add-repo' && a.repo === BILLING.repo, `焦點仍在進行中的「加入」上（實際 ${J(a)}）`);
    await ctx.cdp.pressKey('Enter', 'Enter', 13, '\r');
    const r = await waitAddResponse(ctx, 1, 4000);
    need(r !== null && r.status === 201, `頁面收到延遲的 201（實際 ${r ? r.status : '沒有回應'}）`);
    await sleep(300);
    check(addPosts(ctx).length === 1, `進行中按 Enter 不送出（共 1 筆；實際 ${addPosts(ctx).length}）`);
    check((await snap(ctx)).error === null, '進行中按 Enter 不顯示錯誤');
    await inject(ctx, await craft(ctx, (st) => addProject(st, BILLING, DEFAULT_STAGES.zh)), '含 billing-api 的新投影');
    await inject(ctx, await craft(ctx, backToDetected), 'billing-api 被移除、回到偵測區');
    const b = (await snap(ctx)).repos.find((x) => x.repo === BILLING.repo);
    check(!!b && !!b.button && b.button.ariaDisabled !== 'true', `repo 離開偵測區後解除停用，再出現時「加入」可按（實際 ${J(b && b.button)}）`);
    check(!!b && !!b.button && b.button.text === '加入' && b.button.label === `加入 ${BILLING.name}`, `再出現時文字與 aria-label 回到「加入」（實際 ${J(b && b.button)}）`);
    noExceptions(ctx);
  });
  // 英文介面的進行中文字與 aria-label。
  await withCockpit('inflight-en', { env: { COCKPIT_PREVIEW_WRITE_RULES: '/api/repo-projects=1500:201' } }, async (ctx) => {
    await toggleLanguage(ctx, 'en');
    await clickAdd(ctx, BILLING.repo);
    need((await waitAddRequest(ctx, 1)) !== null, '英文：服務收到 POST /api/repo-projects（回應延遲 1500 ms）');
    const b = await ctx.cdp.run((r1, r2) => {
      const x = window.__rp.addButton(r1);
      const idle = window.__rp.addButton(r2);
      return { text: window.__rp.txt(x), label: x.getAttribute('aria-label'), busy: x.getAttribute('aria-busy'), disabled: x.getAttribute('aria-disabled'), width: x.getBoundingClientRect().width, idleText: window.__rp.txt(idle), idleWidth: idle.getBoundingClientRect().width };
    }, BILLING.repo, DOCS.repo);
    check(b.text === 'Adding…' && b.label === `Adding ${BILLING.name}` && b.busy === 'true' && b.disabled === 'true', `英文：進行中文字為「Adding…」、aria-label 為「Adding ${BILLING.name}」、aria-busy／aria-disabled 皆 "true"（實際 ${J(b)}）`);
    check(b.idleText === 'Add' && Math.abs(b.width - b.idleWidth) < 0.5, `英文：進行中與閒置的按鈕同寬（${b.width} vs ${b.idleWidth}）`);
    noExceptions(ctx);
  });
}

// fix round 1（控制端裁決 Minor 2：使用者明確選擇優先）：201 之後、含新 id 的投影到達之前，使用者手動選了別的
// Project 時，投影到達後不被切走。
async function segManualSelectCancelsAutoSelect() {
  await withCockpit('manual', {}, async (ctx) => {
    await clickAdd(ctx, BILLING.repo);
    const r = await waitAddResponse(ctx, 1);
    need(r !== null && r.status === 201, `頁面收到 201（實際 ${r ? r.status : '沒有回應'}）`);
    await sleep(200);
    await clickProject(ctx, 'p');
    await inject(ctx, await craft(ctx, (st) => addProject(st, BILLING, DEFAULT_STAGES.zh)), '含 billing-api 的新投影');
    const s = await snap(ctx);
    check(s.projects.includes(BILLING.id), `billing-api 出現在 Project 清單（實際 ${J(s.projects)}）`);
    check(s.selected === 'p' && s.shown === 'p', `手動選了 p 之後，含新 id 的投影不把選取切走（實際 selected=${s.selected} shown=${s.shown}）`);
    noExceptions(ctx);
  });
}

// ---------------------------------------------------------------------------
// 段落：cockpit-dashboard「Project 切換」的管理選單、改名、編輯 stage、移除確認（repo-projects task 5.2）
// ---------------------------------------------------------------------------

// ui_preview fixture 的 Repo Project（apply_repo_project_scenario）。
const DEMO = { id: 'demo-app', name: 'Demo App', stages: ['Plan', 'Build'], path: '/api/repo-projects/demo-app' };
const MENU_ITEMS = {
  zh: [['project-rename', '改名'], ['project-edit-stages', '編輯 stage'], ['project-remove', '移除']],
  en: [['project-rename', 'Rename'], ['project-edit-stages', 'Edit stages'], ['project-remove', 'Remove']],
};
const EN_INVALID_STAGES = 'Invalid stages: 1 to 12 stages, each 1 to 32 characters';
const EN_INVALID_NAME = 'Invalid name: after trimming it must be 1 to 64 characters';

const repoWrites = (ctx, method) => ctx.preview.writes.filter((w) => w.method === method && w.path.startsWith('/api/repo-projects/'));
const mgmtWrites = (ctx) => ctx.preview.writes.filter((w) => (w.method === 'PATCH' || w.method === 'DELETE') && w.path.startsWith('/api/repo-projects/'));
// 等服務收到第 n 筆（從 1 起算）method 的 /api/repo-projects/<pid> 請求。
async function waitWrite(ctx, method, n, timeoutMs = UI_TIMEOUT_MS) {
  const start = Date.now();
  for (;;) {
    const list = repoWrites(ctx, method);
    if (list.length >= n) return list[n - 1];
    if (Date.now() - start >= timeoutMs) return null;
    await sleep(50);
  }
}
// 等頁面收到第 n 筆 method 的 /api/repo-projects 回應（responseReceived：204 沒有本體、頁面也不讀，Chrome 不一定送
// loadingFinished）。
async function waitRepoResponse(ctx, method, n, timeoutMs = UI_TIMEOUT_MS) {
  const start = Date.now();
  for (;;) {
    const list = ctx.repoReqs.filter((r) => r.method === method);
    if (list.length >= n && list[n - 1].status !== null) return list[n - 1];
    if (Date.now() - start >= timeoutMs) return null;
    await sleep(50);
  }
}
const dsnap = (ctx) => ctx.cdp.run(() => window.__rp.dialogSnap());
const msnap = (ctx, pid) => ctx.cdp.run((p) => window.__rp.menuSnap(p), pid);
const adesc = (ctx) => ctx.cdp.run(() => window.__rp.activeDesc());
const ESC = (ctx) => ctx.cdp.pressKey('Escape', 'Escape', 27, '');
const TAB = (ctx, shift = false) => ctx.cdp.pressKey('Tab', 'Tab', 9, '', shift ? 8 : 0);
const ENTER = (ctx) => ctx.cdp.pressKey('Enter', 'Enter', 13, '\r');

async function openMenu(ctx, pid) {
  need(await ctx.cdp.clickEl((p) => window.__rp.menuButton(p), [pid], `${pid} 的「⋯」`), `點 ${pid} 的「⋯」`);
  need(!!(await ctx.cdp.poll((p) => !!window.__rp.menu(p), [pid], UI_TIMEOUT_MS)), `${pid} 的選單打開`);
}
async function waitDialog(ctx, kind, label) {
  need(
    !!(await ctx.cdp.poll((k) => { const d = window.__rp.dialogSnap(); return !!d && d.open && d.kind === k; }, [kind], UI_TIMEOUT_MS)),
    `${label}：${kind} 對話框開啟`
  );
}
async function waitDialogClosed(ctx, label) {
  return !!(await ctx.cdp.poll(() => { const d = window.__rp.dialogSnap(); return !!d && !d.open; }, [], UI_TIMEOUT_MS));
}
// 經畫面開啟對話框：點「⋯」→ 點選單項目。
async function openDialog(ctx, pid, action, kind) {
  await openMenu(ctx, pid);
  need(await ctx.cdp.clickEl((p, a) => window.__rp.menuItem(p, a), [pid, action], `選單項目 ${action}`), `點選單項目 ${action}`);
  await waitDialog(ctx, kind, `從 ${pid} 的選單選 ${action}`);
}
async function clickDialog(ctx, op, row = null, desc = null) {
  const label = desc || `對話框的 ${op}${row === null ? '' : `（第 ${row + 1} 列）`}`;
  need(await ctx.cdp.clickEl((o, r) => window.__rp.dialogButton(o, r), [op, row], label), `點${label}`);
}
// 以真的輸入取代輸入框內容：點進去、全選、Input.insertText（空字串改送 Backspace）。which：{ stage: i } 或 { name: true }。
async function typeReplace(ctx, which, text, desc) {
  const locate = (w) => (w.stage !== undefined ? window.__rp.stageInput(w.stage) : window.__rp.nameInput());
  need(await ctx.cdp.clickEl(locate, [which], desc), `點進${desc}`);
  need(
    await ctx.cdp.run((w) => {
      const el = w.stage !== undefined ? window.__rp.stageInput(w.stage) : window.__rp.nameInput();
      if (!el || document.activeElement !== el) return false;
      el.select();
      return true;
    }, which),
    `${desc}取得焦點並全選`
  );
  if (text === '') await ctx.cdp.pressKey('Backspace', 'Backspace', 8, '');
  else await ctx.cdp.send('Input.insertText', { text });
}
function checkFocusOnMenuButton(a, pid, label, focusVisible) {
  check(
    !!a && a.action === 'project-menu' && a.project === pid && (focusVisible === undefined || a.focusVisible === focusVisible),
    `${label}：焦點回到 ${pid} 的「⋯」${focusVisible === undefined ? '' : focusVisible ? '（呈現焦點外框）' : '（不呈現焦點外框）'}（實際 ${J(a)}）`
  );
}
function demoProject(st) {
  return st.projects.find((p) => p.id === DEMO.id);
}

// scenario「只有 Repo Project 有選單」：kind 為 repo 的 demo-app 有「⋯」，手寫的 cockpit、p 沒有；kind 是其他字串
// （未來的新值）也沒有。選單含「改名」「編輯 stage」「移除」；開關選單不送請求、不選定 Project；Esc 與點別處關閉。
async function segOnlyRepoProjectHasMenu() {
  await withCockpit('menu', {}, async (ctx) => {
    for (const lang of ['zh', 'en']) {
      if (lang === 'en') await toggleLanguage(ctx, 'en');
      let m = await msnap(ctx, DEMO.id);
      check(J(m.menuButtonProjects) === J([DEMO.id]), `${lang}：只有 kind 為 repo 的 demo-app 有「⋯」（實際 ${J(m.menuButtonProjects)}）`);
      check(
        !!m.button && m.button.tag === 'BUTTON' && m.button.text === '⋯' && m.button.expanded === 'false' && !m.open,
        `${lang}：「⋯」是 <button>、文字為 ⋯、aria-expanded="false"、選單未開（實際 ${J(m.button)}，open=${m.open}）`
      );
      const wantLabel = lang === 'zh' ? `管理 ${DEMO.name}` : `Manage ${DEMO.name}`;
      check(!!m.button && m.button.label === wantLabel, `${lang}：「⋯」的無障礙名稱為 ${J(wantLabel)}（實際 ${J(m.button && m.button.label)}）`);
      await openMenu(ctx, DEMO.id);
      m = await msnap(ctx, DEMO.id);
      check(
        !!m.button && m.button.expanded === 'true' && !!m.menuId && m.button.controls === m.menuId,
        `${lang}：開啟後 aria-expanded="true"、aria-controls 指向選單（實際 ${J(m.button)}，選單 id ${J(m.menuId)}）`
      );
      check(
        J(m.items.map((i) => [i.action, i.text])) === J(MENU_ITEMS[lang]),
        `${lang}：選單依序為 ${J(MENU_ITEMS[lang].map((x) => x[1]))}（實際 ${J(m.items.map((i) => i.text))}）`
      );
      check(m.items.every((i) => i.tag === 'BUTTON' && i.project === DEMO.id), `${lang}：選單項目都是帶 data-project 的 <button>（實際 ${J(m.items)}）`);
      check((await snap(ctx)).selected === 'cockpit', `${lang}：按「⋯」不改變選定的 Project`);
      // task 6.1 F5：滑鼠按下「⋯」的當下仍停在按鈕上（:hover），展開狀態不能被 hover 蓋掉——邊框與文字維持 --accent。
      const ex = await ctx.cdp.run((p) => {
        const b = window.__rp.menuButton(p);
        const cs = getComputedStyle(b);
        return { hover: b.matches(':hover'), expanded: b.getAttribute('aria-expanded'), border: cs.borderTopColor, color: cs.color, accent: window.__rp.cssColor('accent') };
      }, DEMO.id);
      check(
        ex.hover && ex.expanded === 'true' && ex.border === ex.accent && ex.color === ex.accent,
        `${lang}：展開且 hover 時「⋯」邊框與文字仍是 --accent（實際 ${J(ex)}）`
      );
      await ESC(ctx);
      check(!!(await ctx.cdp.poll((p) => !window.__rp.menu(p), [DEMO.id], UI_TIMEOUT_MS)), `${lang}：Esc 關閉選單`);
      checkFocusOnMenuButton(await adesc(ctx), DEMO.id, `${lang}：Esc 之後`);
      check((await msnap(ctx, DEMO.id)).button.expanded === 'false', `${lang}：關閉後 aria-expanded="false"`);
      // 再按一次「⋯」關閉（切換）。
      await openMenu(ctx, DEMO.id);
      need(await ctx.cdp.clickEl((p) => window.__rp.menuButton(p), [DEMO.id], '再按一次「⋯」'), '再按一次「⋯」');
      check(!!(await ctx.cdp.poll((p) => !window.__rp.menu(p), [DEMO.id], UI_TIMEOUT_MS)), `${lang}：再按一次「⋯」關閉選單`);
      // 點別處（左欄 Project p）關閉選單，且照常選定 p。
      await openMenu(ctx, DEMO.id);
      await clickProject(ctx, 'p');
      check(!(await msnap(ctx, DEMO.id)).open, `${lang}：點左欄別的 Project 時選單關閉`);
      await clickProject(ctx, 'cockpit');
    }
    // kind 為其他字串（未來的新值）不當成 repo。
    await inject(ctx, await craft(ctx, (st) => { st.projects.find((p) => p.id === 'p').kind = 'future-kind'; }), 'p 的 kind 為 future-kind');
    const m = await msnap(ctx, DEMO.id);
    check(J(m.menuButtonProjects) === J([DEMO.id]), `kind 為 future-kind 的 p 沒有「⋯」（實際 ${J(m.menuButtonProjects)}）`);
    check(ctx.preview.writes.length === 0, `開關選單不送任何寫入請求（實際 ${J(ctx.preview.writes.map((w) => `${w.method} ${w.path}`))}）`);
    noExceptions(ctx);
  });
}

// scenario「改名」：從 demo-app 的選單選「改名」，輸入 App 前端並送出 → 服務收到 PATCH /api/repo-projects/demo-app，
// 本體恰為 {"name":"App 前端"}；204 後對話框關閉、焦點回到「⋯」；新投影到達後左欄與 Factory Floor 標題顯示 App 前端。
// 另驗：預填目前名稱、在輸入框按 Enter 也會送出。
async function segRename() {
  await withCockpit('rename', {}, async (ctx) => {
    await clickProject(ctx, DEMO.id);
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    let d = await dsnap(ctx);
    check(d.modal && !d.inApp, `對話框是 #app 之外的 modal <dialog>（實際 modal=${d.modal} inApp=${d.inApp}）`);
    check(d.name === DEMO.name, `名稱輸入框預填目前名稱 ${J(DEMO.name)}（實際 ${J(d.name)}）`);
    check(d.title === `改名：${DEMO.name}`, `task 6.1 F3：改名對話框標題含 Project 名稱（「改名：${DEMO.name}」；實際 ${J(d.title)}）`);
    check(d.activeInDialog && d.active && /project-dialog-name/.test(d.active.cls), `開啟後焦點在名稱輸入框（實際 ${J(d.active)}）`);
    check(!(await msnap(ctx, DEMO.id)).open, '選了選單項目之後選單收起');
    await typeReplace(ctx, { name: true }, 'App 前端', '名稱輸入框');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w = await waitWrite(ctx, 'PATCH', 1);
    need(w !== null, '按「儲存」後服務收到 PATCH');
    check(w.path === DEMO.path, `PATCH 的路徑為 ${DEMO.path}（實際 ${w.path}）`);
    check(w.body === J({ name: 'App 前端' }), `本體恰為 {"name":"App 前端"}（實際 ${w.body}）`);
    const r = await waitRepoResponse(ctx, 'PATCH', 1);
    check(r !== null && r.status === 204, `假端點回 204（實際 ${r ? r.status : '沒有回應'}）`);
    check(await waitDialogClosed(ctx), '成功後對話框關閉');
    checkFocusOnMenuButton(await adesc(ctx), DEMO.id, '改名成功後', false);
    check((await snap(ctx)).error === null, '成功的改名不顯示錯誤訊息');
    // 畫面不自行改名：投影到達前左欄仍是舊名稱。
    const before = await ctx.cdp.run((p) => window.__rp.txt(window.__rp.projectItem(p).querySelector('.project-item-name')), DEMO.id);
    check(before === DEMO.name, `新投影到達前左欄仍顯示舊名稱（實際 ${J(before)}）`);
    await inject(ctx, await craft(ctx, (st) => { demoProject(st).name = 'App 前端'; }), 'demo-app 改名後的投影');
    const names = await ctx.cdp.run((p) => ({
      left: window.__rp.txt(window.__rp.projectItem(p).querySelector('.project-item-name')),
      floor: window.__rp.txt(document.querySelector('[data-region="floor"] .project-name')),
      label: window.__rp.menuButton(p) ? window.__rp.menuButton(p).getAttribute('aria-label') : null,
    }), DEMO.id);
    check(names.left === 'App 前端' && names.floor === 'App 前端', `新投影到達後左欄與 Factory Floor 標題顯示 App 前端（實際 ${J(names)}）`);
    check(names.label === '管理 App 前端', `「⋯」的無障礙名稱跟著新名稱（實際 ${J(names.label)}）`);
    // 在輸入框按 Enter 送出。
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    d = await dsnap(ctx);
    check(d.name === 'App 前端', `再開一次時預填投影中的新名稱（實際 ${J(d.name)}）`);
    await typeReplace(ctx, { name: true }, 'App2', '名稱輸入框');
    await ENTER(ctx);
    const w2 = await waitWrite(ctx, 'PATCH', 2);
    check(w2 !== null && w2.body === J({ name: 'App2' }), `在輸入框按 Enter 送出 {"name":"App2"}（實際 ${w2 ? w2.body : '沒有請求'}）`);
    check(await waitDialogClosed(ctx), 'Enter 送出成功後對話框關閉');
    await sleep(300);
    check(repoWrites(ctx, 'PATCH').length === 2, `每次送出只有一筆 PATCH（實際 ${repoWrites(ctx, 'PATCH').length}）`);
    // task 6.1 F3：英文介面標題為「Rename: <名稱>」。
    await toggleLanguage(ctx, 'en');
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    d = await dsnap(ctx);
    check(d.title === `Rename: ${d.name}`, `英文：改名對話框標題為「Rename: <名稱>」（實際 ${J(d.title)}，輸入框 ${J(d.name)}）`);
    await ESC(ctx);
    noExceptions(ctx);
  });
}

// scenario「編輯 stage」：stages 為 Plan、Implement、Review、Done；把 Implement 改名為 Build、刪除 Review、在最後新增 Ship，
// 按儲存 → PATCH 本體恰為 spec 的那份（from 是原名稱、新增的 from 為 null）。另驗：調整順序、刪除後新增同名（from 為 null）。
async function segEditStages() {
  await withCockpit('stages', {}, async (ctx) => {
    const ORIG = ['Plan', 'Implement', 'Review', 'Done'];
    await inject(ctx, await craft(ctx, (st) => { demoProject(st).stages = ORIG.slice(); }), 'demo-app 的 stages 為 Plan、Implement、Review、Done');
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    let d = await dsnap(ctx);
    check(J(d.stages) === J(ORIG), `對話框依序列出目前的 stages（實際 ${J(d.stages)}）`);
    check(d.title.includes(DEMO.name), `標題含 Project 名稱（實際 ${J(d.title)}）`);
    check(d.activeInDialog && d.active && d.active.row === 0 && /stage-name/.test(d.active.cls), `開啟後焦點在第一個 stage 的輸入框（實際 ${J(d.active)}）`);
    await typeReplace(ctx, { stage: 1 }, 'Build', '第 2 個 stage（Implement）的輸入框');
    await clickDialog(ctx, 'delete', 2, 'Review 那列的「刪除」');
    d = await dsnap(ctx);
    check(J(d.stages) === J(['Plan', 'Build', 'Done']), `改名與刪除後列出 Plan、Build、Done（實際 ${J(d.stages)}）`);
    await clickDialog(ctx, 'add', null, '「新增 stage」');
    d = await dsnap(ctx);
    check(d.stages.length === 4 && d.stages[3] === '', `新增一列空白的 stage（實際 ${J(d.stages)}）`);
    check(d.active && d.active.row === 3 && /stage-name/.test(d.active.cls), `新增後焦點在新的輸入框（實際 ${J(d.active)}）`);
    await ctx.cdp.send('Input.insertText', { text: 'Ship' });
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w = await waitWrite(ctx, 'PATCH', 1);
    need(w !== null, '按「儲存」後服務收到 PATCH');
    const want = { stages: [{ name: 'Plan', from: 'Plan' }, { name: 'Build', from: 'Implement' }, { name: 'Done', from: 'Done' }, { name: 'Ship', from: null }] };
    check(w.path === DEMO.path, `PATCH 的路徑為 ${DEMO.path}（實際 ${w.path}）`);
    check(w.body === J(want), `本體恰為 ${J(want)}（實際 ${w.body}）`);
    check(await waitDialogClosed(ctx), '成功後對話框關閉');
    checkFocusOnMenuButton(await adesc(ctx), DEMO.id, '儲存成功後', false);

    // 調整順序（投影沒變，再開時是原本的四個）：Done 上移兩次、Plan 下移一次。
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    d = await dsnap(ctx);
    check(J(d.stages) === J(ORIG), `再開時依投影列出原本的 stages，不留上次的編輯（實際 ${J(d.stages)}）`);
    const firstUp = await ctx.cdp.run(() => window.__rp.dialogButton('up', 0).getAttribute('aria-disabled'));
    const lastDown = await ctx.cdp.run(() => window.__rp.dialogButton('down', 3).getAttribute('aria-disabled'));
    check(firstUp === 'true' && lastDown === 'true', `第一列的「上移」與最後一列的「下移」為 aria-disabled（實際 ${firstUp}／${lastDown}）`);
    await clickDialog(ctx, 'up', 3, 'Done 的「上移」');
    let a = await adesc(ctx);
    check(!!a && a.op === 'up' && a.row === 2, `上移後焦點跟著 Done 到第 3 列的「上移」（實際 ${J(a)}）`);
    await clickDialog(ctx, 'up', 2, 'Done 的「上移」（第二次）');
    await clickDialog(ctx, 'down', 0, 'Plan 的「下移」');
    d = await dsnap(ctx);
    check(J(d.stages) === J(['Done', 'Plan', 'Implement', 'Review']), `順序為 Done、Plan、Implement、Review（實際 ${J(d.stages)}）`);
    await clickDialog(ctx, 'up', 0, '第一列的「上移」（停用）');
    d = await dsnap(ctx);
    check(J(d.stages) === J(['Done', 'Plan', 'Implement', 'Review']), `按停用的「上移」不改變順序（實際 ${J(d.stages)}）`);
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w2 = await waitWrite(ctx, 'PATCH', 2);
    const want2 = { stages: ['Done', 'Plan', 'Implement', 'Review'].map((n) => ({ name: n, from: n })) };
    check(w2 !== null && w2.body === J(want2), `只調整順序時 from 為各自的名稱（期望 ${J(want2)}，實際 ${w2 ? w2.body : '沒有請求'}）`);
    check(await waitDialogClosed(ctx), '成功後對話框關閉');

    // 刪除 Review 再新增同名的 Review：新增的那列 from 為 null（依列的身分，不依名稱比對）。
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    await clickDialog(ctx, 'delete', 2, 'Review 那列的「刪除」');
    await clickDialog(ctx, 'add', null, '「新增 stage」');
    await ctx.cdp.send('Input.insertText', { text: 'Review' });
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w3 = await waitWrite(ctx, 'PATCH', 3);
    const want3 = { stages: [{ name: 'Plan', from: 'Plan' }, { name: 'Implement', from: 'Implement' }, { name: 'Done', from: 'Done' }, { name: 'Review', from: null }] };
    check(w3 !== null && w3.body === J(want3), `刪除後新增同名的 stage，新增那列 from 為 null（期望 ${J(want3)}，實際 ${w3 ? w3.body : '沒有請求'}）`);
    check(await waitDialogClosed(ctx), '成功後對話框關閉');
    check(repoWrites(ctx, 'PATCH').length === 3, `共 3 筆 PATCH（實際 ${repoWrites(ctx, 'PATCH').length}）`);
    noExceptions(ctx);
  });
}

// scenario「取消編輯與取消移除不送請求」：開「編輯 stage」改了內容後按取消；再選「移除」在確認中按取消；改名按 Esc。
// 服務沒有收到任何 PATCH 或 DELETE；取消後焦點回到「⋯」；再開時不留上次的編輯。
async function segCancelSendsNothing() {
  await withCockpit('cancel', {}, async (ctx) => {
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    await typeReplace(ctx, { stage: 0 }, 'Changed', '第 1 個 stage 的輸入框');
    await clickDialog(ctx, 'add', null, '「新增 stage」');
    await clickDialog(ctx, 'cancel', null, '「取消」');
    check(await waitDialogClosed(ctx), '按「取消」關閉 stage 對話框');
    checkFocusOnMenuButton(await adesc(ctx), DEMO.id, '取消編輯 stage 後', false);
    await openDialog(ctx, DEMO.id, 'project-remove', 'remove');
    const d = await dsnap(ctx);
    check(d.text.includes(DEMO.name), `移除確認含 Project 名稱（實際 ${J(d.text)}）`);
    await clickDialog(ctx, 'cancel', null, '確認中的「取消」');
    check(await waitDialogClosed(ctx), '按「取消」關閉移除確認');
    checkFocusOnMenuButton(await adesc(ctx), DEMO.id, '取消移除後', false);
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    await typeReplace(ctx, { name: true }, 'Nope', '名稱輸入框');
    await ESC(ctx);
    check(await waitDialogClosed(ctx), 'Esc 關閉改名對話框');
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    const again = await dsnap(ctx);
    check(J(again.stages) === J(DEMO.stages), `取消後再開，stages 回到投影的值（實際 ${J(again.stages)}）`);
    await clickDialog(ctx, 'cancel', null, '「取消」');
    await sleep(500);
    check(mgmtWrites(ctx).length === 0, `服務沒有收到任何 PATCH 或 DELETE（實際 ${J(mgmtWrites(ctx).map((w) => `${w.method} ${w.path}`))}）`);
    check((await snap(ctx)).error === null, '取消不顯示錯誤訊息');
    noExceptions(ctx);
  });
}

// scenario「移除要先確認」：選「移除」→ 先顯示含 Project 名稱的確認、尚未送出請求；確認後服務收到 DELETE
// /api/repo-projects/demo-app；新投影到達後 demo-app 從清單消失，原本選定它時改為選定第一個。英文介面同樣顯示名稱。
async function segRemoveNeedsConfirm() {
  await withCockpit('remove', {}, async (ctx) => {
    await clickProject(ctx, DEMO.id);
    await openDialog(ctx, DEMO.id, 'project-remove', 'remove');
    let d = await dsnap(ctx);
    check(d.modal && !d.inApp, `移除確認是 #app 之外的 modal <dialog>（實際 modal=${d.modal} inApp=${d.inApp}）`);
    check(d.text.includes(`「${DEMO.name}」`), `確認文字含「${DEMO.name}」（實際 ${J(d.text)}）`);
    check(d.activeInDialog, `開啟後焦點在確認對話框內（實際 ${J(d.active)}）`);
    await sleep(400);
    check(repoWrites(ctx, 'DELETE').length === 0, '確認之前沒有送出 DELETE');
    await clickDialog(ctx, 'submit', null, '確認中的「移除」');
    const w = await waitWrite(ctx, 'DELETE', 1);
    need(w !== null, '確認後服務收到 DELETE');
    check(w.path === DEMO.path && w.body === '', `DELETE ${DEMO.path}、沒有本體（實際 ${w.path} ${J(w.body)}）`);
    check(await waitDialogClosed(ctx), '成功後確認對話框關閉');
    check((await snap(ctx)).error === null, '成功的移除不顯示錯誤訊息');
    await inject(ctx, await craft(ctx, (st) => { st.projects = st.projects.filter((p) => p.id !== DEMO.id); }), 'demo-app 被移除的投影');
    const s = await snap(ctx);
    check(J(s.projects) === J(['cockpit', 'p']), `demo-app 從 Project 清單消失（實際 ${J(s.projects)}）`);
    check(s.selected === 'cockpit' && s.shown === 'cockpit', `原本選定 demo-app，改為選定第一個 cockpit（實際 selected=${s.selected} shown=${s.shown}）`);
    // fix round 1：204 後焦點在「⋯」上，移除後的投影讓它消失時，焦點落到實際選定的 Project 項目（不掉回 <body>）。
    const a = await adesc(ctx);
    check(!!a && a.action === 'select-project' && a.project === 'cockpit', `「⋯」隨 demo-app 消失後焦點落在選定的 cockpit 項目上（實際 ${J(a)}）`);
    await toggleLanguage(ctx, 'en');
    await openDialog(ctx, DEMO.id, 'project-remove', 'remove');
    d = await dsnap(ctx);
    check(d.text.includes(`"${DEMO.name}"`), `英文確認文字含 "${DEMO.name}"（實際 ${J(d.text)}）`);
    const btn = await ctx.cdp.run(() => window.__rp.txt(window.__rp.dialogButton('submit', null)));
    check(btn === 'Remove', `英文確認鈕為 Remove（實際 ${J(btn)}）`);
    await ESC(ctx);
    check(await waitDialogClosed(ctx), 'Esc 關閉英文確認');
    check(repoWrites(ctx, 'DELETE').length === 1, `共 1 筆 DELETE（實際 ${repoWrites(ctx, 'DELETE').length}）`);
    noExceptions(ctx);
  });
}

// scenario「對話框跨重畫保留」：投影每 100 ms 推送一份新的 version。選單開著時跨重畫仍開；「編輯 stage」對話框開著、
// 已改了一個 stage 名稱但尚未儲存時，2 秒內對話框仍在、內容與焦點保留，接著輸入的字接在後面；移除確認同樣保留。
async function segDialogSurvivesRepaints() {
  await withCockpit('repaint', { env: { COCKPIT_PREVIEW_PUSH_MS: '100' } }, async (ctx) => {
    await openMenu(ctx, DEMO.id);
    let v0 = await stateVersion(ctx);
    let lost = 0;
    for (let i = 0; i < 10; i += 1) {
      await sleep(100);
      const m = await msnap(ctx, DEMO.id);
      const a = await adesc(ctx);
      if (!(m.open && m.button && m.button.expanded === 'true' && a && a.action === 'project-rename')) lost += 1;
    }
    let v1 = await stateVersion(ctx);
    check(Number(v1) - Number(v0) >= 5, `取樣期間至少重畫 5 次（data-state-version ${v0} → ${v1}）`);
    check(lost === 0, `選單跨重畫保持開啟、焦點留在第一個選單項目（10 次取樣有 ${lost} 次不是）`);
    need(await ctx.cdp.clickEl((p, a2) => window.__rp.menuItem(p, a2), [DEMO.id, 'project-edit-stages'], '「編輯 stage」'), '頻繁重畫時點「編輯 stage」');
    await waitDialog(ctx, 'stages', '頻繁重畫時');
    await typeReplace(ctx, { stage: 0 }, 'Des', '第 1 個 stage 的輸入框');
    v0 = await stateVersion(ctx);
    lost = 0;
    for (let i = 0; i < 20; i += 1) {
      await sleep(100);
      const d = await dsnap(ctx);
      if (!(d.open && d.kind === 'stages' && J(d.stages) === J(['Des', 'Build']) && d.active && d.active.row === 0 && /stage-name/.test(d.active.cls))) lost += 1;
    }
    v1 = await stateVersion(ctx);
    check(Number(v1) - Number(v0) >= 10, `2 秒內至少重畫 10 次（data-state-version ${v0} → ${v1}）`);
    check(lost === 0, `對話框跨重畫仍在、已輸入的內容與焦點保留（20 次取樣有 ${lost} 次不是）`);
    await ctx.cdp.send('Input.insertText', { text: 'ign' });
    let d = await dsnap(ctx);
    check(J(d.stages) === J(['Design', 'Build']), `接著輸入的字接在已輸入的內容後面（實際 ${J(d.stages)}）`);
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w = await waitWrite(ctx, 'PATCH', 1);
    const want = { stages: [{ name: 'Design', from: 'Plan' }, { name: 'Build', from: 'Build' }] };
    check(w !== null && w.body === J(want), `頻繁重畫時儲存送出 ${J(want)}（實際 ${w ? w.body : '沒有請求'}）`);
    check(await waitDialogClosed(ctx), '成功後對話框關閉');
    await openDialog(ctx, DEMO.id, 'project-remove', 'remove');
    lost = 0;
    for (let i = 0; i < 10; i += 1) {
      await sleep(100);
      d = await dsnap(ctx);
      if (!(d.open && d.kind === 'remove' && d.activeInDialog && d.text.includes(DEMO.name))) lost += 1;
    }
    check(lost === 0, `移除確認跨重畫保留、焦點留在確認內（10 次取樣有 ${lost} 次不是）`);
    await ESC(ctx);
    check(await waitDialogClosed(ctx), 'Esc 關閉移除確認');
    await sleep(300);
    checkFocusOnMenuButton(await adesc(ctx), DEMO.id, '頻繁重畫時關閉確認後');
    check(repoWrites(ctx, 'DELETE').length === 0, '取消的移除沒有送出 DELETE');
    noExceptions(ctx);
  });
}

// 鍵盤可操作：Enter 開選單（焦點到第一個項目）、Tab 到「編輯 stage」Enter 開對話框（焦點進入對話框）、Tab／Shift+Tab
// 在對話框內循環不跑出去、Esc 關閉且焦點回到「⋯」並呈現焦點外框；移除確認同樣。滑鼠開啟後 Esc 同樣回到「⋯」。
async function segDialogKeyboard() {
  await withCockpit('keyboard', {}, async (ctx) => {
    await TAB(ctx);
    need(await ctx.cdp.run((p) => { const b = window.__rp.menuButton(p); if (!b) return false; b.focus(); return document.activeElement === b; }, DEMO.id), '焦點移到 demo-app 的「⋯」');
    await ENTER(ctx);
    need(!!(await ctx.cdp.poll((p) => !!window.__rp.menu(p), [DEMO.id], UI_TIMEOUT_MS)), '在「⋯」上按 Enter 打開選單');
    let a = await adesc(ctx);
    check(!!a && a.action === 'project-rename' && a.focusVisible === true, `鍵盤開啟選單後焦點在「改名」上、呈現焦點外框（實際 ${J(a)}）`);
    await TAB(ctx);
    a = await adesc(ctx);
    check(!!a && a.action === 'project-edit-stages', `Tab 移到「編輯 stage」（實際 ${J(a)}）`);
    await ENTER(ctx);
    await waitDialog(ctx, 'stages', '在「編輯 stage」上按 Enter');
    let d = await dsnap(ctx);
    check(d.activeInDialog && d.active.row === 0 && /stage-name/.test(d.active.cls), `焦點進入對話框的第一個輸入框（實際 ${J(d.active)}）`);
    const n = await ctx.cdp.run(() => Array.from(window.__rp.dialog().querySelectorAll('input, button')).filter((x) => !x.disabled && x.getClientRects().length > 0).length);
    need(n >= 4, `對話框內可聚焦的元素至少 4 個（實際 ${n}）`);
    let outside = 0;
    const seen = [];
    for (let i = 0; i < n; i += 1) {
      await TAB(ctx);
      d = await dsnap(ctx);
      if (!d.activeInDialog) outside += 1;
      seen.push(d.active ? `${d.active.op || d.active.cls}@${d.active.row}` : null);
    }
    check(outside === 0, `連按 ${n} 次 Tab 焦點都在對話框內（${outside} 次跑出去；${J(seen)}）`);
    check(d.active && d.active.row === 0 && /stage-name/.test(d.active.cls), `按完一圈回到第一個輸入框（實際 ${J(d.active)}）`);
    await TAB(ctx, true);
    d = await dsnap(ctx);
    check(d.activeInDialog && d.active && d.active.op === 'submit', `在第一個輸入框按 Shift+Tab 移到最後的「儲存」（實際 ${J(d.active)}）`);
    await ESC(ctx);
    check(await waitDialogClosed(ctx), 'Esc 關閉 stage 對話框');
    checkFocusOnMenuButton(await adesc(ctx), DEMO.id, '鍵盤 Esc 關閉後', true);
    // 移除確認：Enter → Tab Tab → Enter；Tab 不跑出去；Esc 回到「⋯」。
    await ENTER(ctx);
    need(!!(await ctx.cdp.poll((p) => !!window.__rp.menu(p), [DEMO.id], UI_TIMEOUT_MS)), '再按 Enter 打開選單');
    await TAB(ctx);
    await TAB(ctx);
    a = await adesc(ctx);
    check(!!a && a.action === 'project-remove', `Tab 兩次到「移除」（實際 ${J(a)}）`);
    await ENTER(ctx);
    await waitDialog(ctx, 'remove', '在「移除」上按 Enter');
    outside = 0;
    for (let i = 0; i < 4; i += 1) {
      await TAB(ctx);
      if (!(await dsnap(ctx)).activeInDialog) outside += 1;
    }
    check(outside === 0, `移除確認內連按 4 次 Tab 焦點都在確認內（${outside} 次跑出去）`);
    await ESC(ctx);
    check(await waitDialogClosed(ctx), 'Esc 關閉移除確認');
    checkFocusOnMenuButton(await adesc(ctx), DEMO.id, '移除確認 Esc 之後', true);
    // 滑鼠開啟後 Esc：焦點回「⋯」、不呈現外框。
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    await ESC(ctx);
    check(await waitDialogClosed(ctx), 'Esc 關閉改名對話框');
    // Esc 本身是鍵盤輸入，焦點外框依最後輸入方式呈現（專案 memory：滑鼠之後的程式 focus 才傳 focusVisible:false），這裡不驗外框。
    checkFocusOnMenuButton(await adesc(ctx), DEMO.id, '滑鼠開啟、Esc 關閉後');
    check(mgmtWrites(ctx).length === 0, `只開關對話框不送 PATCH／DELETE（實際 ${mgmtWrites(ctx).length}）`);
    noExceptions(ctx);
  });
}

// 對話框的基本提示（送出前）：空白的 stage 名稱、重複的 stage 名稱、沒有任何 stage、空白的 Project 名稱 → 不送出、在對話框
// 內顯示原因、焦點移到該欄。長度等其餘規則交給後端（前端不複算後端規則）：65 字元的名稱照樣送出。
async function segDialogValidation() {
  await withCockpit('validate', {}, async (ctx) => {
    const BAD = await ctx.cdp.run(() => window.__rp.cssColor('bad'));
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    await typeReplace(ctx, { stage: 0 }, '   ', '第 1 個 stage 的輸入框');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    let d = await dsnap(ctx);
    check(d.open && d.error !== null && /空白/.test(d.error), `空白的 stage 名稱：不送出、對話框內顯示原因（實際 ${J(d.error)}）`);
    check(d.active && d.active.row === 0 && /stage-name/.test(d.active.cls), `焦點移到空白的那一欄（實際 ${J(d.active)}）`);
    // task 6.1 F4：出錯的那一欄 aria-invalid="true"、aria-describedby 指向錯誤訊息、邊框用 --bad；其他欄不受影響。
    check(
      d.active.invalid === 'true' && !!d.active.describedby && d.active.describedText === d.error && d.active.borderColor === BAD,
      `空白 stage：出錯的輸入框 aria-invalid="true"、aria-describedby 指向錯誤訊息、邊框為 --bad（實際 ${J(d.active)}）`
    );
    check((await ctx.cdp.run(() => window.__rp.invalidInputs())) === 1, '空白 stage：只有出錯的那一欄帶 aria-invalid／aria-describedby');
    await typeReplace(ctx, { stage: 0 }, ' Build ', '第 1 個 stage 的輸入框');
    check((await ctx.cdp.run(() => window.__rp.invalidInputs())) === 0, '修正（輸入）後出錯的輸入框移除 aria-invalid／aria-describedby');
    check((await adesc(ctx)).borderColor !== BAD, '修正後邊框不再是 --bad');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    d = await dsnap(ctx);
    check(d.open && d.error !== null && /重複/.test(d.error) && d.error.includes('Build'), `重複的 stage 名稱（去除前後空白後相同）：不送出、顯示原因含名稱（實際 ${J(d.error)}）`);
    check(
      d.active && d.active.invalid === 'true' && d.active.describedText === d.error && /stage-name/.test(d.active.cls),
      `重複 stage：出錯的輸入框 aria-invalid="true"、aria-describedby 指向錯誤訊息（實際 ${J(d.active)}）`
    );
    await clickDialog(ctx, 'delete', 0, '第 1 列的「刪除」');
    await clickDialog(ctx, 'delete', 0, '第 1 列的「刪除」（第二次）');
    d = await dsnap(ctx);
    check(d.stages.length === 0, `刪光所有 stage（實際 ${J(d.stages)}）`);
    check(d.activeInDialog, `刪掉焦點所在的列後焦點仍在對話框內（實際 ${J(d.active)}）`);
    await clickDialog(ctx, 'submit', null, '「儲存」');
    d = await dsnap(ctx);
    check(d.open && d.error !== null && /至少/.test(d.error), `沒有任何 stage：不送出、顯示原因（實際 ${J(d.error)}）`);
    check((await ctx.cdp.run(() => window.__rp.invalidInputs())) === 0, '沒有任何 stage 的提示不指向任何輸入框（沒有輸入框可標）');
    await sleep(300);
    check(repoWrites(ctx, 'PATCH').length === 0, `提示期間沒有送出 PATCH（實際 ${repoWrites(ctx, 'PATCH').length}）`);
    check((await snap(ctx)).error === null, '送出前的提示不寫進頁面的錯誤橫幅');
    await clickDialog(ctx, 'add', null, '「新增 stage」');
    await ctx.cdp.send('Input.insertText', { text: 'X' });
    d = await dsnap(ctx);
    check(d.error === null, `修正後提示消失（實際 ${J(d.error)}）`);
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w = await waitWrite(ctx, 'PATCH', 1);
    check(w !== null && w.body === J({ stages: [{ name: 'X', from: null }] }), `修正後照常送出（實際 ${w ? w.body : '沒有請求'}）`);
    check(await waitDialogClosed(ctx), '成功後對話框關閉');
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    await typeReplace(ctx, { name: true }, '', '名稱輸入框');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    d = await dsnap(ctx);
    check(d.open && d.error !== null && /空白/.test(d.error), `空白的名稱：不送出、顯示原因（實際 ${J(d.error)}）`);
    check(
      d.active && /project-dialog-name/.test(d.active.cls) && d.active.invalid === 'true' && d.active.describedText === d.error && d.active.borderColor === BAD,
      `task 6.1 F4：空白的名稱：名稱輸入框 aria-invalid="true"、aria-describedby 指向錯誤訊息、邊框為 --bad（實際 ${J(d.active)}）`
    );
    await sleep(300);
    check(repoWrites(ctx, 'PATCH').length === 1, `空白名稱沒有送出 PATCH（實際 ${repoWrites(ctx, 'PATCH').length}）`);
    const long = 'n'.repeat(65);
    await typeReplace(ctx, { name: true }, long, '名稱輸入框');
    check((await ctx.cdp.run(() => window.__rp.invalidInputs())) === 0, '名稱修正（輸入）後移除 aria-invalid／aria-describedby');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    const w2 = await waitWrite(ctx, 'PATCH', 2);
    check(w2 !== null && w2.body === J({ name: long }), `65 字元的名稱照樣送出，長度規則交給後端（實際 ${w2 ? w2.body.length : '沒有請求'}）`);
    noExceptions(ctx);
  });
}

// scenario「被拒絕時顯示原因」與「錯誤代碼可翻譯」（管理端點）：PATCH 回 400 invalid_stages 時對話框不關、已編輯的內容
// 保留、對話框與頁面錯誤橫幅都顯示原因（繁中為回應的 error，英文依 code 用字典的英文訊息）；取消後橫幅仍在。
// 改名的 400 invalid_name、移除的 409 也依同樣規則顯示。
async function segManageRejected() {
  const rules = `${DEMO.path}=0:400:invalid_stages`;
  await withCockpit('patchrejected', { env: { COCKPIT_PREVIEW_WRITE_RULES: rules } }, async (ctx) => {
    for (const lang of ['zh', 'en']) {
      if (lang === 'en') await toggleLanguage(ctx, 'en');
      await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
      await typeReplace(ctx, { stage: 1 }, 'Ship', '第 2 個 stage 的輸入框');
      await clickDialog(ctx, 'submit', null, '「儲存」');
      const n = lang === 'zh' ? 1 : 2;
      need((await waitWrite(ctx, 'PATCH', n)) !== null, `${lang}：服務收到 PATCH（回 400 invalid_stages）`);
      const r = await waitRepoResponse(ctx, 'PATCH', n);
      check(r !== null && r.status === 400, `${lang}：頁面收到 400（實際 ${r ? r.status : '沒有回應'}）`);
      const ok = await ctx.cdp.poll(() => { const d = window.__rp.dialogSnap(); return !!d && d.error !== null && d.error !== undefined; }, [], UI_TIMEOUT_MS);
      const d = await dsnap(ctx);
      const s = await snap(ctx);
      if (lang === 'zh') {
        check(!!ok && /400/.test(d.error) && /ui_preview 模擬回應 400/.test(d.error), `繁中：對話框顯示含回應 error 的原因（實際 ${J(d.error)}）`);
        check(s.error !== null && /ui_preview 模擬回應 400/.test(s.error), `繁中：頁面錯誤橫幅顯示同一個原因（實際 ${J(s.error)}）`);
      } else {
        check(!!ok && d.error.includes(EN_INVALID_STAGES) && !/模擬/.test(d.error), `英文：對話框依 code 顯示英文原因（實際 ${J(d.error)}）`);
        check(s.error !== null && s.error.includes(EN_INVALID_STAGES), `英文：頁面錯誤橫幅依 code 顯示英文原因（實際 ${J(s.error)}）`);
      }
      check(d.open && J(d.stages) === J(['Plan', 'Ship']), `${lang}：失敗時對話框不關、已編輯的內容保留（實際 open=${d.open} ${J(d.stages)}）`);
      const busy = await ctx.cdp.run(() => window.__rp.dialogButton('submit', null).getAttribute('aria-disabled'));
      check(busy !== 'true', `${lang}：失敗後「儲存」恢復可按（實際 aria-disabled=${busy}）`);
      await clickDialog(ctx, 'cancel', null, '「取消」');
      check(await waitDialogClosed(ctx), `${lang}：按「取消」關閉`);
      check((await snap(ctx)).error !== null, `${lang}：關閉對話框後錯誤橫幅仍在（直到下一次操作或使用者關閉）`);
      await inject(ctx, await craft(ctx, () => {}), `${lang}：一般投影`);
      check((await snap(ctx)).error !== null, `${lang}：之後的重畫不清除錯誤橫幅`);
    }
    // 移除被拒絕：確認對話框內顯示原因（同一條規則，英文依 code）。
    await openDialog(ctx, DEMO.id, 'project-remove', 'remove');
    await clickDialog(ctx, 'submit', null, '確認中的「Remove」');
    need((await waitWrite(ctx, 'DELETE', 1)) !== null, '服務收到 DELETE（回 400）');
    const ok = await ctx.cdp.poll(() => { const d = window.__rp.dialogSnap(); return !!d && d.error !== null; }, [], UI_TIMEOUT_MS);
    const d = await dsnap(ctx);
    check(!!ok && d.open && d.error.includes(EN_INVALID_STAGES), `移除被拒絕時確認不關、顯示英文原因（實際 open=${d.open} ${J(d.error)}）`);
    await ESC(ctx);
    noExceptions(ctx);
  });
  await withCockpit('renamerejected', { env: { COCKPIT_PREVIEW_WRITE_RULES: `${DEMO.path}=0:400:invalid_name` } }, async (ctx) => {
    await toggleLanguage(ctx, 'en');
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    await typeReplace(ctx, { name: true }, 'x'.repeat(65), 'name input');
    await clickDialog(ctx, 'submit', null, '"Save"');
    need((await waitWrite(ctx, 'PATCH', 1)) !== null, '服務收到 PATCH（回 400 invalid_name）');
    const ok = await ctx.cdp.poll(() => { const d = window.__rp.dialogSnap(); return !!d && d.error !== null; }, [], UI_TIMEOUT_MS);
    const d = await dsnap(ctx);
    check(!!ok && d.open && d.error.includes(EN_INVALID_NAME), `英文：改名被拒絕時對話框依 code 顯示英文原因（實際 ${J(d.error)}）`);
    check(d.name === 'x'.repeat(65), '失敗時已輸入的名稱保留');
    noExceptions(ctx);
  });
}

// 送出進行中不重送：回應延遲 4000 ms（fix round 1 起：要容得下進行中的取消與 Esc 操作），期間「儲存」呈忙碌（aria-disabled＋aria-busy），再按或按 Enter 都不送第二筆；
// 回應到達後對話框關閉。
async function segSubmitInFlight() {
  await withCockpit('submitflight', { env: { COCKPIT_PREVIEW_WRITE_RULES: `${DEMO.path}=4000:204` } }, async (ctx) => {
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    await typeReplace(ctx, { name: true }, 'Slow', '名稱輸入框');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    need((await waitWrite(ctx, 'PATCH', 1)) !== null, '服務收到 PATCH（回應延遲 4000 ms）');
    const btn = await ctx.cdp.run(() => { const b = window.__rp.dialogButton('submit', null); return { dis: b.getAttribute('aria-disabled'), busy: b.getAttribute('aria-busy'), attr: b.hasAttribute('disabled') }; });
    check(btn.dis === 'true' && btn.busy === 'true' && btn.attr === false, `進行中「儲存」為 aria-disabled＋aria-busy、沒有 disabled 屬性（實際 ${J(btn)}）`);
    await clickDialog(ctx, 'submit', null, '進行中再按「儲存」');
    // fix round 1：請求已送出，進行中不能「取消」——「取消」為 aria-disabled、按了與 Esc 都不關閉對話框。
    const cancelDis = await ctx.cdp.run(() => window.__rp.dialogButton('cancel', null).getAttribute('aria-disabled'));
    check(cancelDis === 'true', `進行中「取消」為 aria-disabled（實際 ${cancelDis}）`);
    await clickDialog(ctx, 'cancel', null, '進行中按「取消」');
    await ESC(ctx);
    await ESC(ctx);
    const still = await dsnap(ctx);
    check(still.open && still.kind === 'rename', `進行中按「取消」與連按 Esc 都不關閉對話框（實際 open=${still.open}）`);
    need(await ctx.cdp.run(() => { const i = window.__rp.nameInput(); i.focus(); return document.activeElement === i; }), '焦點回到名稱輸入框');
    await ENTER(ctx);
    const r = await waitRepoResponse(ctx, 'PATCH', 1, 8000);
    check(r !== null && r.status === 204, `頁面收到延遲的 204（實際 ${r ? r.status : '沒有回應'}）`);
    check(await waitDialogClosed(ctx), '回應到達後對話框關閉');
    await sleep(300);
    check(repoWrites(ctx, 'PATCH').length === 1, `進行中再按不送出第二筆（共 ${repoWrites(ctx, 'PATCH').length} 筆）`);
    check((await snap(ctx)).error === null, '沒有錯誤訊息');
    // 請求結束後「取消」恢復可按。
    await openDialog(ctx, DEMO.id, 'project-remove', 'remove');
    const cancelAfter = await ctx.cdp.run(() => window.__rp.dialogButton('cancel', null).getAttribute('aria-disabled'));
    check(cancelAfter !== 'true', `請求結束後再開啟，「取消」可按（實際 aria-disabled=${cancelAfter}）`);
    await clickDialog(ctx, 'cancel', null, '「取消」');
    check(await waitDialogClosed(ctx), '「取消」關閉移除確認');
    noExceptions(ctx);
  });
}

// scenario「名稱不以 HTML 解讀」（選單與對話框部分）：Repo Project 名稱與 stage 名稱含 HTML 時，「⋯」的無障礙名稱、對話框
// 標題、stage 輸入框、移除確認都以原樣文字呈現，沒有建立任何 <img>、沒有執行指令碼。
async function segDialogNamesNotHtml() {
  await withCockpit('dialoghtml', {}, async (ctx) => {
    const EVIL = '<img src=x onerror=window.__rpPwned=1>';
    const EVIL_STAGE = '<b>x</b><img src=y onerror=window.__rpPwned=2>';
    const imgsBefore = await ctx.cdp.run(() => document.querySelectorAll('img').length);
    await inject(ctx, await craft(ctx, (st) => { const p = demoProject(st); p.name = EVIL; p.stages = [EVIL_STAGE, 'Plan']; }), '名稱與 stage 含 HTML 的 demo-app');
    const m = await msnap(ctx, DEMO.id);
    check(!!m.button && m.button.label === `管理 ${EVIL}`, `「⋯」的無障礙名稱是原樣文字（實際 ${J(m.button && m.button.label)}）`);
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    let d = await dsnap(ctx);
    check(d.title.includes(EVIL), `stage 對話框標題以原樣文字含名稱（實際 ${J(d.title)}）`);
    check(J(d.stages) === J([EVIL_STAGE, 'Plan']), `stage 輸入框的值是原樣文字（實際 ${J(d.stages)}）`);
    await clickDialog(ctx, 'cancel', null, '「取消」');
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    d = await dsnap(ctx);
    check(d.name === EVIL, `改名輸入框預填原樣文字（實際 ${J(d.name)}）`);
    await clickDialog(ctx, 'cancel', null, '「取消」');
    await openDialog(ctx, DEMO.id, 'project-remove', 'remove');
    d = await dsnap(ctx);
    check(d.text.includes(EVIL), `移除確認以原樣文字含名稱（實際 ${J(d.text)}）`);
    await sleep(300);
    const dom = await ctx.cdp.run(() => ({ imgs: document.querySelectorAll('img').length, pwned: window.__rpPwned === undefined ? null : window.__rpPwned }));
    check(dom.imgs === imgsBefore && dom.pwned === null, `頁面沒有多建立任何 <img>、沒有執行指令碼（之前 ${imgsBefore} 個；實際 ${J(dom)}）`);
    await clickDialog(ctx, 'cancel', null, '「取消」');
    noExceptions(ctx);
  });
}

// fix round 1：焦點所在的 Project 從投影消失（被別處移除）時，焦點落到實際選定的 Project 項目；沒有任何 Project 時落到
// 偵測區標題，不落在該 repo 的「加入」（連按 Enter 會立刻重新加入）。涵蓋開著的選單項目、開著的對話框按「取消」、「⋯」。
async function segFocusWhenProjectVanishes() {
  await withCockpit('vanish', {}, async (ctx) => {
    const demoOrig = (await ctx.cdp.run(() => window.__rp.latest())).projects.find((p) => p.id === DEMO.id);
    need(!!demoOrig && typeof demoOrig.repo === 'string', '取得 fixture 中 demo-app 的投影（含 repo key）');
    const without = (st) => { st.projects = st.projects.filter((p) => p.id !== DEMO.id); };
    const back = async (label) => inject(ctx, await craft(ctx, (st) => {
      if (!st.projects.some((p) => p.id === DEMO.id)) st.projects.push(JSON.parse(JSON.stringify(demoOrig)));
    }), label);
    // 選單開著、焦點在「改名」上時 demo-app 消失。
    await clickProject(ctx, 'p');
    await openMenu(ctx, DEMO.id);
    await inject(ctx, await craft(ctx, without), '選單開著時 demo-app 消失');
    let a = await adesc(ctx);
    check(!(await msnap(ctx, DEMO.id)).open, '選單隨 Project 消失而收起');
    check(!!a && a.action === 'select-project' && a.project === 'p', `選單項目消失後焦點落在選定的 p 項目上（實際 ${J(a)}）`);
    // 對話框開著時 demo-app 消失，按「取消」。
    await back('demo-app 回來');
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    await inject(ctx, await craft(ctx, without), '對話框開著時 demo-app 消失');
    await clickDialog(ctx, 'cancel', null, '「取消」');
    check(await waitDialogClosed(ctx), '按「取消」關閉對話框');
    a = await adesc(ctx);
    check(!!a && a.action === 'select-project' && a.project === 'p', `「⋯」已不在時，取消後焦點落在選定的 p 項目上（實際 ${J(a)}）`);
    // 沒有任何 Project：焦點在「⋯」上時所有 Project 消失、demo-app 的 repo 回到偵測區最前面。
    await back('demo-app 回來');
    await TAB(ctx);
    need(await ctx.cdp.run((p) => { const b = window.__rp.menuButton(p); if (!b) return false; b.focus(); return document.activeElement === b; }, DEMO.id), '焦點移到 demo-app 的「⋯」');
    await inject(ctx, await craft(ctx, (st) => {
      st.projects = [];
      st.detected_repos.unshift({ repo: demoOrig.repo, name: demoOrig.name, pane_count: 2 });
    }), '所有 Project 消失、demo-app 回到偵測區');
    a = await adesc(ctx);
    check(!!a && /detected-repos-title/.test(a.cls), `沒有任何 Project 時焦點落在偵測區標題（實際 ${J(a)}）`);
    await ENTER(ctx);
    await sleep(300);
    check(addPosts(ctx).length === 0, `在落點按 Enter 不會送出加入（實際 ${addPosts(ctx).length} 筆）`);
    noExceptions(ctx);
  });
}

// fix round 2：沒有任何 Project、焦點落到偵測區標題後，之後的整頁重畫仍把焦點還原到新的標題節點（不掉回 <body>）。
// 注入的投影擋掉了真的推送，所以在頁面裡每 100 ms 以 onState 重送一份 version＋1 的同一份投影，製造頻繁重畫。
async function segDetectedTitleFocusSurvivesRepaints() {
  await withCockpit('titlefocus', { env: { COCKPIT_PREVIEW_PUSH_MS: '100' } }, async (ctx) => {
    await TAB(ctx);
    need(await ctx.cdp.run((p) => { const b = window.__rp.menuButton(p); if (!b) return false; b.focus(); return document.activeElement === b; }, DEMO.id), '焦點移到 demo-app 的「⋯」');
    await inject(ctx, await craft(ctx, (st) => { st.projects = []; }), '所有 Project 消失');
    const isTitle = () => {
      const t = document.querySelector('[data-region="projects"] .detected-repos-title');
      return !!t && document.activeElement === t;
    };
    need(await ctx.cdp.run(isTitle), '焦點落在偵測區標題');
    // task 6.1 F2：鍵盤輸入之後的程式焦點呈現 :focus-visible，外框圓角 4px（與按鈕一致）；沒有焦點時不加圓角。
    const fv = await ctx.cdp.run(() => {
      const t = document.querySelector('[data-region="projects"] .detected-repos-title');
      const cs = getComputedStyle(t);
      return { focusVisible: t.matches(':focus-visible'), radius: cs.borderTopLeftRadius, outline: cs.outlineStyle + ' ' + cs.outlineWidth, offset: cs.outlineOffset };
    });
    check(fv.focusVisible && fv.radius === '4px' && fv.outline === 'solid 2px', `標題取得焦點外框時圓角 4px、仍是 2px 實線外框（實際 ${J(fv)}）`);
    await ctx.cdp.run(() => {
      window.__rpRepaintTimer = setInterval(() => {
        const st = JSON.parse(JSON.stringify(window.cockpitLatestState()));
        st.version = Number(st.version) + 1;
        window.__rpOrigOnState(st);
      }, 100);
      return true;
    });
    const v0 = await stateVersion(ctx);
    let lost = 0;
    for (let i = 0; i < 20; i += 1) {
      await sleep(100);
      if (!(await ctx.cdp.run(isTitle))) lost += 1;
    }
    const v1 = await stateVersion(ctx);
    await ctx.cdp.run(() => { clearInterval(window.__rpRepaintTimer); return true; });
    check(Number(v1) - Number(v0) >= 10, `2 秒內至少重畫 10 次（data-state-version ${v0} → ${v1}）`);
    check(lost === 0, `重畫期間 document.activeElement 始終是（新的）偵測區標題節點（20 次取樣有 ${lost} 次不是）`);
    noExceptions(ctx);
  });
}

// fix round 1：對話框開啟後，投影中的 stages 在別處被改了（或 Project 已被移除）時，送出前擋下、在對話框內提示，不送請求
// （避免以過期的 from 覆寫別處的修改）。
async function segStaleDialogBlocked() {
  await withCockpit('stale', {}, async (ctx) => {
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    await typeReplace(ctx, { stage: 0 }, 'Design', '第 1 個 stage 的輸入框');
    await inject(ctx, await craft(ctx, (st) => { demoProject(st).stages = ['Plan', 'Build', 'Ship']; }), '別處把 demo-app 的 stages 改成 Plan、Build、Ship');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    let d = await dsnap(ctx);
    check(d.open && d.error !== null && /別處/.test(d.error), `stages 已在別處變更：不送出、對話框內提示（實際 ${J(d.error)}）`);
    check(J(d.stages) === J(['Design', 'Build']), `已編輯的內容保留（實際 ${J(d.stages)}）`);
    await clickDialog(ctx, 'cancel', null, '「取消」');
    await openDialog(ctx, DEMO.id, 'project-edit-stages', 'stages');
    d = await dsnap(ctx);
    check(J(d.stages) === J(['Plan', 'Build', 'Ship']), `重新開啟後列出最新的 stages（實際 ${J(d.stages)}）`);
    await clickDialog(ctx, 'cancel', null, '「取消」');
    await openDialog(ctx, DEMO.id, 'project-rename', 'rename');
    await typeReplace(ctx, { name: true }, 'Gone', '名稱輸入框');
    await inject(ctx, await craft(ctx, (st) => { st.projects = st.projects.filter((p) => p.id !== DEMO.id); }), '改名對話框開著時 demo-app 被移除');
    await clickDialog(ctx, 'submit', null, '「儲存」');
    d = await dsnap(ctx);
    check(d.open && d.error !== null && /不存在/.test(d.error), `Project 已不存在：不送出、對話框內提示（實際 ${J(d.error)}）`);
    await sleep(300);
    check(mgmtWrites(ctx).length === 0, `都沒有送出 PATCH／DELETE（實際 ${J(mgmtWrites(ctx).map((w) => `${w.method} ${w.path}`))}）`);
    await clickDialog(ctx, 'cancel', null, '「取消」');
    noExceptions(ctx);
  });
}

// repo-projects task 5.3：spec cockpit-dashboard「Repo Project 工作線的 worktree 標註」與「畫面操作」的
// 「固定 pane 的工作線沒有改綁鈕」。demo-app 的 win~wJ:p7 帶 worktree "demo-app-wt"、win~wJ:p6 沒有，兩者的
// binding.source 都是 pane：列首不得有「改綁」「取消改綁」，綁定摘要沒有「改綁」徽章；手寫 project 的 auto／override
// 列照舊（否定對照）。另驗：worktree 以文字節點呈現（不當 HTML）、中英文案、整頁重畫後仍正確、source 為 pane 的
// 各種綁定狀態（bound／unbound／runtime_disconnected）都沒有改綁鈕、改綁模式下仍可進出且不受影響。
const WT_ROW = { project: 'demo-app', wt: 'win~wJ:p7', plain: 'win~wJ:p6', worktree: 'demo-app-wt' };
const rowSnap = (ctx, pid, wid) =>
  ctx.cdp.run(
    (p, w) => {
      const proj = document.querySelector(`[data-region="floor"] .project[data-project="${p}"]`);
      if (!proj) return { project: false };
      const h = Array.from(proj.querySelectorAll('.ff-row-header')).find((x) => x.getAttribute('data-workstream') === w);
      if (!h) return { project: true, row: false };
      const txt = (e) => (e ? (e.textContent || '').replace(/\s+/g, ' ').trim() : null);
      const wt = h.querySelector('.ff-worktree');
      return {
        project: true,
        row: true,
        name: txt(h.querySelector('.ff-ws-name')),
        worktree: wt ? txt(wt) : null,
        worktreeTitle: wt ? wt.getAttribute('title') : null,
        worktreeChildren: wt ? wt.children.length : 0,
        worktreeCount: h.querySelectorAll('.ff-worktree').length,
        binding: txt(h.querySelector('.ff-binding-text')),
        badge: txt(h.querySelector('.ff-binding-badge')),
        actions: Array.from(h.querySelectorAll('[data-action]')).map((b) => b.getAttribute('data-action')),
        actionTexts: Array.from(h.querySelectorAll('[data-action]')).map((b) => txt(b)),
        injected: h.querySelectorAll('b,i,img,script').length,
      };
    },
    pid,
    wid
  );
const WT_TEXT = {
  zh: { title: 'worktree：demo-app-wt', rebind: '改綁', undo: '取消改綁', view: '看輸出' },
  en: { title: 'Worktree: demo-app-wt', rebind: 'Rebind', undo: 'Undo rebind', view: 'View output' },
};

function checkFixtureRows(lang, wt, plain, label) {
  const T = WT_TEXT[lang];
  check(!!wt.row, `${label}：demo-app 有 win~wJ:p7 這一列`);
  check(wt.name === 'codex', `${label}：win~wJ:p7 的名稱為 codex（實際 ${J(wt.name)}）`);
  check(wt.worktree === WT_ROW.worktree, `${label}：win~wJ:p7 列首顯示 worktree 標註 ${WT_ROW.worktree}（實際 ${J(wt.worktree)}）`);
  check(wt.worktreeCount === 1, `${label}：win~wJ:p7 列首恰有一個 worktree 標註（實際 ${wt.worktreeCount}）`);
  check(wt.worktreeTitle === T.title, `${label}：worktree 標註的 title 為 ${J(T.title)}（實際 ${J(wt.worktreeTitle)}）`);
  check(wt.worktreeChildren === 0, `${label}：worktree 標註是單一文字節點（實際子元素 ${wt.worktreeChildren}）`);
  check(wt.badge === null, `${label}：win~wJ:p7 綁定摘要沒有「改綁」徽章（實際 ${J(wt.badge)}）`);
  check(!wt.actions.includes('rebind') && !wt.actions.includes('override-clear'), `${label}：win~wJ:p7 列首沒有 rebind／override-clear 按鈕（實際 ${J(wt.actions)}）`);
  check(!wt.actionTexts.includes(T.rebind) && !wt.actionTexts.includes(T.undo), `${label}：win~wJ:p7 列首文字也沒有「${T.rebind}」「${T.undo}」（實際 ${J(wt.actionTexts)}）`);
  check(J(wt.actions) === J(['select-bound-pane']), `${label}：win~wJ:p7 列首只剩「${T.view}」（實際 ${J(wt.actions)}）`);
  check(!!plain.row, `${label}：demo-app 有 win~wJ:p6 這一列`);
  check(plain.name === 'api-worker', `${label}：win~wJ:p6 的名稱為 api-worker（實際 ${J(plain.name)}）`);
  check(plain.worktree === null && plain.worktreeCount === 0, `${label}：win~wJ:p6（主 worktree）沒有 worktree 標註（實際 ${J(plain.worktree)}）`);
  check(plain.badge === null, `${label}：win~wJ:p6 綁定摘要沒有「改綁」徽章（實際 ${J(plain.badge)}）`);
  check(J(plain.actions) === J(['select-bound-pane']), `${label}：win~wJ:p6 列首沒有「改綁」與「取消改綁」，只剩「${T.view}」（實際 ${J(plain.actions)}）`);
}

async function segWorktreeLabelAndPaneNoRebind() {
  await withCockpit('worktree', {}, async (ctx) => {
    await clickProject(ctx, WT_ROW.project);
    for (const lang of ['zh', 'en']) {
      if (lang === 'en') {
        await toggleLanguage(ctx, 'en');
        await clickProject(ctx, WT_ROW.project);
      }
      const wt = await rowSnap(ctx, WT_ROW.project, WT_ROW.wt);
      const plain = await rowSnap(ctx, WT_ROW.project, WT_ROW.plain);
      checkFixtureRows(lang, wt, plain, lang);
    }
    // 否定對照：手寫 project 的 auto／override 列照舊有「改綁」，沒有 worktree 標註。
    await clickProject(ctx, 'cockpit');
    const manual = await ctx.cdp.run(() => {
      const rows = Array.from(document.querySelectorAll('[data-region="floor"] .project .ff-row-header'));
      return rows.map((h) => ({
        id: h.getAttribute('data-workstream'),
        actions: Array.from(h.querySelectorAll('[data-action]')).map((b) => b.getAttribute('data-action')),
        worktree: !!h.querySelector('.ff-worktree'),
      }));
    });
    const latest = await ctx.cdp.run(() => window.__rp.latest());
    const cockpit = latest.projects.find((p) => p.id === 'cockpit');
    const manualRows = cockpit.workstreams.filter((w) => w.binding.source === 'auto' || w.binding.source === 'override');
    check(manualRows.length > 0, `手寫 project cockpit 有 source 為 auto 或 override 的工作線（實際 ${J(cockpit.workstreams.map((w) => w.binding.source))}）`);
    const withSource = manual.filter((r) => manualRows.some((w) => w.id === r.id));
    check(withSource.length === manualRows.length && withSource.every((r) => r.actions.includes('rebind')), `手寫 project 的 auto／override 列照舊有「改綁」鈕（實際 ${J(manual)}）`);
    check(manual.every((r) => !r.worktree), '手寫 project 的列沒有 worktree 標註');

    // 注入各種 source／worktree 組合。
    await clickProject(ctx, WT_ROW.project);
    const hostile = '<b>x</b><img src=x onerror=1>';
    await inject(
      ctx,
      await craft(ctx, (st) => {
        const p = st.projects.find((x) => x.id === WT_ROW.project);
        const base = JSON.parse(JSON.stringify(p.workstreams.find((w) => w.id === WT_ROW.wt)));
        const mk = (id, name, binding, worktree) => {
          const w = JSON.parse(JSON.stringify(base));
          w.id = id;
          w.name = name;
          w.binding = binding;
          if (worktree === undefined) delete w.worktree;
          else w.worktree = worktree;
          p.workstreams.push(w);
        };
        mk('win~x:hostile', 'hostile', { state: 'bound', runtime: 'win', pane_id: 'wJ:p6', source: 'pane', agent: 'claude', agent_status: 'idle' }, hostile);
        mk('win~x:unbound', 'unbound', { state: 'unbound', runtime: 'win', source: 'pane' }, 'wt-u');
        mk('win~x:disc', 'disc', { state: 'runtime_disconnected', runtime: 'win', source: 'pane' });
        mk('win~x:override', 'ovr', { state: 'bound', runtime: 'win', pane_id: 'wJ:p6', source: 'override', agent: 'claude', agent_status: 'idle' });
        mk('win~x:auto', 'auto', { state: 'bound', runtime: 'win', pane_id: 'wJ:p6', source: 'auto', agent: 'claude', agent_status: 'idle' });
        mk('win~x:empty', 'empty', { state: 'bound', runtime: 'win', pane_id: 'wJ:p6', source: 'pane', agent: 'claude', agent_status: 'idle' }, '');
      }),
      '依 source／worktree 組合補幾條工作線'
    );
    const rows = {};
    for (const id of ['hostile', 'unbound', 'disc', 'override', 'auto', 'empty']) rows[id] = await rowSnap(ctx, WT_ROW.project, `win~x:${id}`);
    check(rows.hostile.worktree === hostile && rows.hostile.injected === 0, `worktree 以文字節點呈現、不當 HTML（顯示 ${J(rows.hostile.worktree)}，注入元素 ${rows.hostile.injected}）`);
    check(!rows.hostile.actions.includes('rebind'), `pane／bound 沒有「改綁」（實際 ${J(rows.hostile.actions)}）`);
    check(rows.unbound.worktree === 'wt-u' && !rows.unbound.actions.includes('rebind') && !rows.unbound.actions.includes('override-clear'), `pane／unbound 有 worktree、沒有「改綁」「取消改綁」（實際 ${J(rows.unbound)}）`);
    check(!rows.disc.actions.includes('rebind') && !rows.disc.actions.includes('override-clear') && rows.disc.badge === null && rows.disc.worktree === null, `pane／runtime_disconnected 沒有「改綁」「取消改綁」與徽章（實際 ${J(rows.disc)}）`);
    check(J(rows.override.actions) === J(['select-bound-pane', 'rebind', 'override-clear']) && rows.override.badge === 'Rebound', `override 照舊：「改綁」＋「取消改綁」＋徽章（實際 ${J(rows.override)}）`);
    check(J(rows.auto.actions) === J(['select-bound-pane', 'rebind']) && rows.auto.badge === null, `auto 照舊：只有「改綁」、沒有徽章（實際 ${J(rows.auto)}）`);
    check(rows.empty.worktree === null && rows.empty.worktreeCount === 0, `worktree 為空字串時不顯示標註（實際 ${J(rows.empty.worktree)}）`);

    // 改綁模式：auto 列進入改綁模式後整頁重畫，pane 工作線仍沒有改綁鈕、模式仍在。
    need(
      await ctx.cdp.clickEl(() => document.querySelector('.ff-row-header[data-workstream="win~x:auto"] [data-action="rebind"]'), [], 'auto 列的「改綁」'),
      '點 auto 列的「改綁」進入改綁模式'
    );
    await inject(ctx, await craft(ctx, () => {}), '改綁模式下整頁重畫');
    const inMode = await ctx.cdp.run(() => ({
      banner: !!document.querySelector('.rebind-banner'),
      bindHere: document.querySelectorAll('[data-action="bind-here"]').length,
    }));
    check(inMode.banner && inMode.bindHere > 0, `改綁模式跨重畫仍在、pane 列有「綁定到這裡」（實際 ${J(inMode)}）`);
    const after = await rowSnap(ctx, WT_ROW.project, WT_ROW.wt);
    check(!after.actions.includes('rebind') && after.worktree === WT_ROW.worktree, `改綁模式重畫後 win~wJ:p7 仍有 worktree 標註、沒有「改綁」（實際 ${J(after.actions)}）`);

    // 一般整頁重畫後仍正確（目前語言為 en）。
    for (let i = 0; i < 3; i += 1) await inject(ctx, await craft(ctx, () => {}), `重畫第 ${i + 1} 次`);
    const wt2 = await rowSnap(ctx, WT_ROW.project, WT_ROW.wt);
    const plain2 = await rowSnap(ctx, WT_ROW.project, WT_ROW.plain);
    checkFixtureRows('en', wt2, plain2, '重畫後');
    check(ctx.preview.writes.length === 0, `整段沒有送出任何寫入請求（實際 ${J(ctx.preview.writes.map((w) => `${w.method} ${w.path}`))}）`);
    noExceptions(ctx);
  });
}

const SEGMENTS = [
  { code: 'self/段落代號', fn: segSelfSegmentArg, self: true },
  { code: 'cockpit-dashboard/列出偵測到的 repo', fn: segListDetected },
  { code: 'cockpit-dashboard/按加入送出預設 stages', fn: segAddSendsDefaultStages },
  { code: 'cockpit-dashboard/加入成功後自動選定新 Project', fn: segAutoSelectAfterAdd },
  { code: 'cockpit-dashboard/加入回應晚於新投影時仍自動選定', fn: segAutoSelectWhenResponseIsLate },
  { code: 'cockpit-dashboard/加入被拒絕時不自動選定', fn: segNoAutoSelectWhenRejected },
  { code: 'cockpit-dashboard/頻繁重畫時加入鈕的鍵盤焦點', fn: segKeyboardFocusAcrossRepaints },
  { code: 'cockpit-dashboard/空狀態指向偵測到的 repo', fn: segEmptyStatePointsToDetected },
  { code: 'cockpit-dashboard/沒有偵測到的 repo', fn: segNoDetectedRepos },
  { code: 'cockpit-dashboard/偵測區名稱不以 HTML 解讀', fn: segDetectedNameNotHtml },
  { code: 'cockpit-dashboard/連點兩下加入只送一筆', fn: segDoubleClickSendsOnce },
  { code: 'cockpit-dashboard/加入進行中不重送且跨重畫保留', fn: segAddInFlightAcrossRepaints },
  { code: 'cockpit-dashboard/投影到達前手動切換則不自動選定', fn: segManualSelectCancelsAutoSelect },
  { code: 'cockpit-dashboard/只有 Repo Project 有選單', fn: segOnlyRepoProjectHasMenu },
  { code: 'cockpit-dashboard/改名', fn: segRename },
  { code: 'cockpit-dashboard/編輯 stage', fn: segEditStages },
  { code: 'cockpit-dashboard/取消編輯與取消移除不送請求', fn: segCancelSendsNothing },
  { code: 'cockpit-dashboard/移除要先確認', fn: segRemoveNeedsConfirm },
  { code: 'cockpit-dashboard/對話框跨重畫保留', fn: segDialogSurvivesRepaints },
  { code: 'cockpit-dashboard/對話框鍵盤操作與焦點回歸', fn: segDialogKeyboard },
  { code: 'cockpit-dashboard/對話框送出前的基本提示', fn: segDialogValidation },
  { code: 'cockpit-dashboard/管理操作被拒絕時顯示原因', fn: segManageRejected },
  { code: 'cockpit-dashboard/送出進行中不重送', fn: segSubmitInFlight },
  { code: 'cockpit-dashboard/對話框名稱不以 HTML 解讀', fn: segDialogNamesNotHtml },
  { code: 'cockpit-dashboard/Project 消失時焦點落在選定的 Project', fn: segFocusWhenProjectVanishes },
  { code: 'cockpit-dashboard/stages 已在別處變更時不送出', fn: segStaleDialogBlocked },
  { code: 'cockpit-dashboard/偵測區標題的焦點跨重畫保留', fn: segDetectedTitleFocusSurvivesRepaints },
  { code: 'cockpit-dashboard/Repo Project 工作線的 worktree 標註與固定 pane 無改綁鈕', fn: segWorktreeLabelAndPaneNoRebind },
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
  // 環境檢查：不動別人的行程，有衝突就停。
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
  if (others.length > 0) log(`注意：已有其他 ui_preview.exe 在執行（PID ${JSON.stringify(others)}），不是本腳本開的，不處理；計時斷言可能受負載影響`);
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
  const alive = [...OUR_PIDS].filter(pidStillRunning);
  check(alive.length === 0, `結束後本腳本 spawn 過的行程都已不存在（殘留 PID ${JSON.stringify(alive)}）`);
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
