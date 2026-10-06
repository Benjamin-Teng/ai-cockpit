// split-check.js：file-split-view 前端驗收腳本（file-split-view task 2.1 建立骨架，後續 task 往上加段落）。
// headless Chrome＋raw CDP。啟動、收尾、行程所有權模型、段落代號與輸出格式沿用
// docs/research/2026-09-27/files-check.js（startPreview／startChrome／killTree＋tasklist 收尾判準、
// 「還握著 ChildProcess 且沒觀察到 exit 才終止」、parseSegmentArg、打錯段名 exit 2）。
//
// 對 `cockpit --example ui_preview` 驗 openspec/specs/file-review/spec.md「檔案並排」等需求中屬於前端的
// scenario，以及「沒有並排時」的基準行為（重構前後都必須綠）。
//
// 用法（repo 根；先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-10-04/split-check.js                              # 全部段落
//   node docs/research/2026-10-04/split-check.js "self/鷹架,baseline/"        # 只跑指定段落（逗號分隔；`<前綴>/` 選該前綴全部）
// 段落代號拼錯、空字串或只有逗號 → 印 `RESULT: FAIL (段落代號)`、exit 2，不啟動任何行程。
//
// 段落代號：
//   self/鷹架                                  純函式自我測試（中繼資料網址解析、請求分類）
//   self/段落代號                              命令列段落代號驗證
//   baseline/沒有並排時只有目前分頁可見        基準：兩個檔案分頁時只有目前分頁可見，只有它被查中繼資料
//   file-review/快速切換時舊回應丟棄           自動更新：連點 B→A→B，A 卡住的舊查詢被中止，不蓋到 A 或 B（task 2.3）
//   file-review/關閉分頁中止進行中的查詢       自動更新：關閉分頁時它卡住的查詢被中止，之後不再查（task 2.3）
//   file-review/加入並排 … 並排中各欄輪詢互不影響  檔案並排的狀態轉換與並排中的輪詢（task 3.1，共 13 段；
//                                              清單與做法見 SEGMENTS 與 split-check.md 段落代號表）
//   file-review/並排版面等寬 … 切換 Project 不影響並排  並排版面（task 3.2，共 5 段）：1280 寬的等寬與欄位順序、三欄加長行
//                                              與 PDF 不撐破頁面、iframe 不重新載入、整頁重畫與切換 Project 不影響並排。
//                                              另外 expectSplit() 每次都呼叫 checkLayout()，3.1 的段落也一併驗版面
//   file-review/Ctrl＋點選加入並排 … 關閉並排的焦點欄時接手分頁捲進視野  並排鈕、Ctrl＋點選、Ctrl＋Enter、欄位標記與
//                                              無障礙（task 3.3，共 7 段）；另外 3.3 擴充了「沒有另一個檔案分頁時不並排」
//                                              （停用呈現、Git Graph 為目前分頁）與「非檔案分頁不能並排」（.review-tab-split
//                                              也算並排鈕、diff 分頁、Ctrl＋點選 Git Graph）
//   file-review/不在並排中移出成員時分頁列不捲動  task 3.3 修正第 1 輪（審查 Minor 1）
//   file-review/在欄內點選切換焦點欄 … 兩個 html 欄之間直接切換焦點欄  在並排欄內按下滑鼠切換焦點欄（task 3.4，共 5 段；
//                                              design D6）：欄內容、md 相對連結、PDF「下一頁」、html 欄的 iframe 與工具列
//                                              （iframe 靠 window blur，task 3.4 修正第 1 輪）、兩個 html 欄的 iframe 之間直接
//                                              切換與輪詢計時器的啟停（修正第 2 輪）
//   file-review/窄視窗只顯示焦點欄 … 窄視窗下按下滑鼠不切換焦點欄  窄視窗（task 3.5，共 4 段；design D5）：以
//                                              Emulation.setDeviceMetricsOverride 在 1280 與 700 之間切換，驗顯示的欄數、
//                                              服務收到的中繼資料查詢只涵蓋可見的分頁（含中止與立即查詢）、窄視窗下替換
//                                              焦點欄、窄視窗下按下滑鼠不切換焦點欄
//   file-review/還原並排 … 寫入格式與回滾相容  持久化並排組合（task 3.6，共 6 段；design D8）：還原並排、選定 Live Output
//                                              時重新整理、並排資料不合法時忽略（spec 五種＋越界＋同一檔案兩筆）、舊格式（v1 與
//                                              沒有 split 的 v2）、焦點欄不在並排組合中時改用第一欄、寫入格式（略過不存的分頁後才
//                                              算索引、沒有並排組合時不寫）與回滾相容（上一個發行版 v0.1.2 的 isStoredState()）
//   file-review/設計審核修正的外觀  設計審核修正（task 4.1，1 段）：reading-flow 讓 Tab 順序與欄位編號一致、三欄時工具列路徑不被壓扁、
//                                              焦點欄的 1px 外框（不被裁掉）、非焦點欄的徽章用 --text-dim、800 寬 PDF 按鈕不逐字直排
//
// 新增段落的方式（後續 task）：寫一個 `async function segXxx()`，用 withCockpit() 開一組 preview＋chrome，
// 斷言用 check()／need()，最後在檔尾 SEGMENTS 加一列 `{ code: '<capability>/<scenario 名稱>', fn: segXxx }`，
// 並同步更新 split-check.md 的段落代號表。spec scenario 的段落用 `file-review/` 前綴。
//
// 埠：preview 從 7910 起、CDP 從 19510 起（pickPort 遇到占用就往上找）。這兩段沒有被其他驗收腳本用過
// （2026-10-04 grep docs/research/*/*.js 的 pickPort／PORT 常數：preview 用過 7770、7780、7792、7793、7830、7870、
// 7970、7990；CDP 用過 18781–18991、19000–19200、19310、19410–19440、9333）。開跑前 7910 或 19510 已有人
// LISTEN 就直接結束（exit 2），不搶、不動別人的行程。不使用也不碰 7770（那是使用者的 cockpit 或別的腳本），
// 有人 LISTEN 時只印一行「注意」。
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

const PREVIEW_PORT_BASE = 7910;
const CDP_PORT_BASE = 19510;
// 同 files-check.js：推送間隔拉長，讓背景輪替在段落期間靜止；需要頻繁重畫的段落以 env 覆寫。
const STABLE_PUSH_MS = '600000';
const CHROME_UDD_PREFIX = 'cockpit-chrome-splitcheck-';
const PREVIEW_TEMP_PREFIX = 'cockpit-ui-preview-';
const RUNTIME = 'win';
const PANE_REVIEW = 'wJ:p4'; // cwd＝review-repo/src（spec 的 w1:p1）
const UI_TIMEOUT_MS = 5000;
const POLL_INTERVAL_MS = 2000; // files.js 的 POLL_INTERVAL_MS（spec「自動更新」：前一次結束後 2 秒）
// CDP Input 的 modifiers 位元（Chrome DevTools Protocol Input.dispatchMouseEvent／dispatchKeyEvent）。
const MOD = { ALT: 1, CTRL: 2, META: 4, SHIFT: 8 };

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
// 斷言與輸出（同 files-check.js）
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

// ---------------------------------------------------------------------------
// 行程管理（同 files-check.js；只終止本腳本自己 spawn、還握著 ChildProcess 且沒觀察到 exit 的行程）
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
// CDP（同 files-check.js 的 CDP class；clickEl／pressKey 多了 modifiers，給 Ctrl＋點選、Ctrl＋Enter 用）
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
  // 點擊 fn(...args) 回傳的元素：捲進可視範圍、以中心點命中測試確認點得到它（或其子孫），再送真的滑鼠
  // 事件。opts.modifiers：MOD 位元（例如 MOD.CTRL）。
  async clickEl(fn, args, desc, opts = {}) {
    const modifiers = opts.modifiers || 0;
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
    const base = { x: pt.x, y: pt.y, button: 'left', clickCount: 1, modifiers };
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: pt.x, y: pt.y, modifiers });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
    return true;
  }
  // 真的鍵盤事件（頁面 keydown 監聽器看得到 event.key／ctrlKey）。Ctrl＋Enter：
  // pressKey('Enter', 'Enter', 13, '\r', MOD.CTRL)。
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

const PREVIEW_TEMP_ROOTS = new Set();
async function startPreview(envOverrides, label) {
  if (!fs.existsSync(UI_PREVIEW_EXE)) {
    throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  }
  const port = pickPort(PREVIEW_PORT_BASE);
  const info = { reviewRepo: null, otherRepo: null };
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
  const preview = { server, port, ...info, tempRoot: info.reviewRepo ? path.dirname(info.reviewRepo) : null };
  if (preview.tempRoot) PREVIEW_TEMP_ROOTS.add(preview.tempRoot);
  if (!(up && info.reviewRepo && info.otherRepo)) {
    check(false, `${label} 應該在 10 秒內開始回應並印出 review-repo／other-repo 路徑（up=${up}，review-repo=${info.reviewRepo}，other-repo=${info.otherRepo}）`);
    await stopPreview(preview, `${label}（啟動失敗收尾）`);
    throw new Error(`${label} 沒有起來`);
  }
  return preview;
}

// ui_preview 以 taskkill /F 結束時沒有機會自己清暫存目錄，由這裡刪；只刪名稱符合 ui_preview 前綴、
// 且是本次啟動印出的那一個目錄。
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
// 頁面端工具：以 Page.addScriptToEvaluateOnNewDocument 安裝成 window.__sc（重新整理後仍在）。
// 定位規則沿用 files-check.js 的前端契約 C1–C3（files-check.md「前端契約」）。
// ---------------------------------------------------------------------------

function pageHelpers() {
  if (window.__sc) return;
  const txt = (el) => {
    if (!el) return '';
    let t = el.innerText;
    if (typeof t !== 'string') t = el.textContent || '';
    return t.replace(/\s+/g, ' ').trim();
  };
  // 畫面上看得到：在文件中、沒有被 display:none／visibility:hidden 藏起來、有版面框。
  const visible = (el) => {
    if (!el || !el.isConnected) return false;
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden') return false;
    return el.getClientRects().length > 0;
  };
  const filesRoot = () => document.getElementById('files');
  const reviewRoot = () => document.getElementById('review');
  const leftTablist = () => (filesRoot() ? filesRoot().querySelector('[role="tablist"]') : null);
  const leftTabs = () => (leftTablist() ? Array.from(leftTablist().querySelectorAll('[role="tab"]')) : []);
  const leftTab = (name) => leftTabs().find((t) => txt(t) === name) || null;
  const tree = () => (filesRoot() ? filesRoot().querySelector('[role="tree"]') : null);
  const rows = () => (tree() ? Array.from(tree().querySelectorAll('[role="treeitem"]')) : []);
  const row = (p) => rows().find((r) => r.getAttribute('title') === p) || null;
  const reviewTablist = () => (reviewRoot() ? reviewRoot().querySelector('[role="tablist"]') : null);
  const reviewTabs = () => (reviewTablist() ? Array.from(reviewTablist().querySelectorAll('[role="tab"]')) : []);
  const isLive = (t) => /Live Output/.test(txt(t));
  const liveTab = () => reviewTabs().find(isLive) || null;
  const fileTab = (p) => reviewTabs().find((t) => t.getAttribute('data-path') === p) || null;
  const selectedTab = () => reviewTabs().find((t) => t.getAttribute('aria-selected') === 'true') || null;
  // 分頁名稱：Live Output 為 'LIVE'、檔案分頁為相對路徑、Git Graph 分頁為 'GRAPH'（file-split-view task 3.1）。
  // file-split-view task 3.3：diff 分頁為 'DIFF:<相對路徑>'。
  const tabName = (t) => {
    if (!t) return null;
    if (isLive(t)) return 'LIVE';
    if (t.hasAttribute('data-path')) return t.getAttribute('data-path');
    if (t.hasAttribute('data-graph-root')) return 'GRAPH';
    if (t.hasAttribute('data-diff-path')) return `DIFF:${t.getAttribute('data-diff-path')}`;
    return t.id;
  };
  const panelOf = (t) => {
    if (!t) return null;
    const id = t.getAttribute('aria-controls');
    const p = id ? document.getElementById(id) : null;
    return p && p.getAttribute('role') === 'tabpanel' ? p : null;
  };
  // 分頁區的可見狀態快照：每個 tab 的選定與其 tabpanel 的 hidden／實際可見；另列出 #review 內沒有被任何
  // tab 認領的 tabpanel（不該存在，存在就是 aria-controls 斷掉）。
  const reviewState = () => {
    const tabs = reviewTabs();
    const owned = new Set();
    const list = tabs.map((t) => {
      const p = panelOf(t);
      if (p) owned.add(p);
      return { name: tabName(t), selected: t.getAttribute('aria-selected') === 'true', panel: p ? p.id : null, hidden: p ? p.hidden : null, visible: visible(p) };
    });
    const root = reviewRoot();
    const orphans = root ? Array.from(root.querySelectorAll('[role="tabpanel"]')).filter((p) => !owned.has(p)).map((p) => ({ id: p.id, visible: visible(p) })) : [];
    return {
      selected: tabName(selectedTab()),
      tabs: list,
      orphans,
      split: root ? root.getAttribute('data-split') : null,
    };
  };
  // 投影版本（整頁重畫的判斷依據；見檔頭）。
  const stateVersion = () => {
    const v = document.getElementById('version');
    return v && v.hasAttribute('data-state-version') ? v.getAttribute('data-state-version') : null;
  };
  // 並排鈕（file-split-view task 3.1；design D7）：分頁的包裝元素（.review-tab，分頁的父元素，不是分頁列本身）裡
  // 帶 aria-pressed 的 <button>。Live Output 分頁直接放在分頁列裡，沒有包裝元素，所以一定回 null。
  const splitButtonFor = (t) => {
    if (!t) return null;
    const w = t.parentElement;
    if (!w || w === reviewTablist()) return null;
    return w.querySelector('button[aria-pressed]');
  };
  // 包裝元素裡所有「看起來是並排鈕」的元素（file-split-view task 3.3；審查第 2 項）：帶 aria-pressed 的 <button>，
  // 或 class 為 .review-tab-split 的元素。兩種都算，git 類分頁誤長出沒有 aria-pressed 的並排鈕也抓得到。
  const splitButtonsIn = (t) => {
    if (!t) return [];
    const w = t.parentElement;
    if (!w || w === reviewTablist()) return [];
    return Array.from(w.querySelectorAll('button[aria-pressed], .review-tab-split'));
  };
  const graphTab = () => reviewTabs().find((t) => t.hasAttribute('data-graph-root')) || null;
  const diffTab = (p) => reviewTabs().find((t) => t.getAttribute('data-diff-path') === p) || null;
  // 依 tabName() 的名稱找分頁（'LIVE'、'GRAPH'、'DIFF:<路徑>' 或檔案相對路徑）。
  const tabByName = (name) => reviewTabs().find((t) => tabName(t) === name) || null;
  // 並排狀態快照（file-split-view task 3.1）：
  //   cols：並排組合依欄位順序的分頁名稱（讀包裝元素的 data-split-col，1 起算）；colNums：對應的欄位編號。
  //   pressed：每個有並排鈕的分頁的 aria-pressed；buttons：每個分頁有沒有並排鈕（splitButtonsIn()，task 3.3 起
  //   帶 aria-pressed 的 <button> 與 .review-tab-split 都算）。
  //   selected：aria-selected="true" 的分頁；selectedCount：幾個分頁 aria-selected="true"。
  //   shown：tabpanel 實際可見的分頁（分頁列順序）；names：全部分頁（分頁列順序）。
  const splitState = () => {
    const tabs = reviewTabs();
    const cols = [];
    const pressed = {};
    const buttons = {};
    for (const t of tabs) {
      const w = t.parentElement;
      const col = w && w !== reviewTablist() ? w.getAttribute('data-split-col') : null;
      if (col !== null) cols.push({ col: Number(col), name: tabName(t) });
      const b = splitButtonFor(t);
      buttons[tabName(t)] = splitButtonsIn(t).length > 0;
      if (b) pressed[tabName(t)] = b.getAttribute('aria-pressed');
    }
    cols.sort((x, y) => x.col - y.col);
    return {
      cols: cols.map((c) => c.name),
      colNums: cols.map((c) => c.col),
      pressed,
      buttons,
      selected: tabName(selectedTab()),
      selectedCount: tabs.filter((t) => t.getAttribute('aria-selected') === 'true').length,
      shown: tabs.filter((t) => visible(panelOf(t))).map(tabName),
      names: tabs.map(tabName),
    };
  };
  // 並排版面的幾何快照（file-split-view task 3.2；design D4）：
  //   split：#review 的 data-split；review：#review 的寬與左右邊界；gap：#review 的 column-gap（px）。
  //   panels：實際可見的 tabpanel（分頁列順序），各自的名稱、版面框、是否帶 data-split-focus、上緣框線顏色，以及面板本身
//           是否橫向溢出（scrollWidth − clientWidth）與右緣超出面板內緣的元素（檢視器捲動容器 .file-viewer-host 裡的
//           不算，那裡的內容本來就在容器內捲動）。
  //   focusMarked：#review 內所有帶 data-split-focus 的元素（對應的分頁名稱，或元素 id）。
  //   accent：--accent token 換成 rgb() 字串（和 getComputedStyle 的顏色字串同格式）。
  //   page：頁面是否有橫向捲軸（documentElement 的 scrollWidth 大於 clientWidth）與 innerWidth。
  const layoutState = () => {
    const root = reviewRoot();
    const rr = root.getBoundingClientRect();
    const owner = new Map();
    reviewTabs().forEach((t) => {
      const p = panelOf(t);
      if (p) owner.set(p, tabName(t));
    });
    const panels = reviewTabs()
      .map((t) => ({ name: tabName(t), p: panelOf(t) }))
      .filter((x) => x.p && visible(x.p))
      .map((x) => {
        const r = x.p.getBoundingClientRect();
        const host = x.p.querySelector('.file-viewer-host');
        const innerRight = r.left + x.p.clientLeft + x.p.clientWidth;
        const spill = Array.from(x.p.querySelectorAll('*'))
          .filter((el) => !(host && host.contains(el)) && el.getClientRects().length > 0 && el.getBoundingClientRect().right > innerRight + 0.5)
          .map((el) => `${el.tagName.toLowerCase()}.${String(el.className).replace(/ /g, '.')}@${Math.round(el.getBoundingClientRect().right - innerRight)}px`);
        return {
          overflow: x.p.scrollWidth - x.p.clientWidth,
          spill,
          name: x.name,
          left: r.left,
          right: r.right,
          top: r.top,
          width: r.width,
          height: r.height,
          focus: x.p.hasAttribute('data-split-focus'),
          border: getComputedStyle(x.p).borderTopColor,
        };
      });
    const focusMarked = Array.from(root.querySelectorAll('[data-split-focus]')).map((el) => owner.get(el) || el.id || el.tagName);
    const hex = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim();
    const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
    const accent = m ? `rgb(${parseInt(m[1], 16)}, ${parseInt(m[2], 16)}, ${parseInt(m[3], 16)})` : hex;
    const de = document.documentElement;
    return {
      split: root.getAttribute('data-split'),
      review: { left: rr.left, right: rr.right, width: rr.width },
      gap: parseFloat(getComputedStyle(root).columnGap) || 0,
      panels,
      focusMarked,
      accent,
      page: { hScroll: de.scrollWidth > de.clientWidth, scrollWidth: de.scrollWidth, clientWidth: de.clientWidth, innerWidth: window.innerWidth },
    };
  };
  // #review 直屬子節點的順序（分頁列與各 tabpanel 的 id）：並排不得搬動任何面板的 DOM（design D4）。
  const reviewChildOrder = () => Array.from(reviewRoot().children).map((el) => el.id || el.className);
  // 並排鈕與欄位標記的呈現快照（file-split-view task 3.3；design D7），以分頁名稱為鍵：
  //   buttons：包裝元素裡並排鈕的個數（splitButtonsIn）；以下欄位都取第一個並排鈕，沒有時為 null。
  //   pressed／ariaDisabled／title／ariaLabel：屬性原值；disabledAttr：是否帶 HTML 的 disabled 屬性（應該不帶）；
  //   tabIndex：並排鈕的 tabIndex；shown：畫面上看得到（display 不是 none、visibility 為 visible、opacity ≥ 0.99、
  //   有版面框）；opacity：計算後的 opacity。
  //   col：包裝元素的 data-split-col；desc：分頁 aria-describedby 指到的每個元素的文字（找不到的為 null）；
  //   badge：分頁按鈕 ::after 計算後的 content（沒有徽章時為 'none'）；badgeDisplay：::after 的 display。
  //   tabTabIndex：分頁本身的 tabIndex；tabTitle：分頁的 title；label：分頁內 .review-tab-label 的文字。
  const splitUi = () => {
    const out = {};
    for (const t of reviewTabs()) {
      const w = t.parentElement && t.parentElement !== reviewTablist() ? t.parentElement : null;
      const btns = splitButtonsIn(t);
      const b = btns[0] || null;
      let shown = null;
      let opacity = null;
      if (b) {
        const cs = getComputedStyle(b);
        opacity = parseFloat(cs.opacity);
        shown = cs.display !== 'none' && cs.visibility === 'visible' && opacity >= 0.99 && b.getClientRects().length > 0;
      }
      const ids = (t.getAttribute('aria-describedby') || '').split(/\s+/).filter((s) => s !== '');
      const after = getComputedStyle(t, '::after');
      const labelEl = t.querySelector('.review-tab-label');
      out[tabName(t)] = {
        buttons: btns.length,
        pressed: b ? b.getAttribute('aria-pressed') : null,
        ariaDisabled: b ? b.getAttribute('aria-disabled') : null,
        disabledAttr: b ? b.hasAttribute('disabled') : null,
        title: b ? b.getAttribute('title') : null,
        ariaLabel: b ? b.getAttribute('aria-label') : null,
        tabIndex: b ? b.tabIndex : null,
        shown,
        opacity,
        col: w ? w.getAttribute('data-split-col') : null,
        desc: ids.map((id) => {
          const el = document.getElementById(id);
          return el ? el.textContent.trim() : null;
        }),
        badge: after.content,
        badgeDisplay: after.display,
        tabTabIndex: t.tabIndex,
        tabTitle: t.getAttribute('title'),
        label: labelEl ? labelEl.textContent : null,
      };
    }
    return out;
  };
  // 輸入事件記錄（file-split-view task 3.3）：在 document 的捕捉階段記下每個 keydown（key、ctrlKey、目標）與每個
  // click（落在哪個分頁上、ctrlKey）。takeInput() 取出並清空。用來驗 CDP 送的修飾鍵真的到達頁面，以及 Ctrl＋Enter
  // 之後沒有再多一次 click（<button> 的 Enter 預設轉成 click）。
  const installInputLog = () => {
    if (window.__scInput) return true;
    window.__scInput = { keys: [], clicks: [] };
    document.addEventListener('keydown', (e) => {
      window.__scInput.keys.push({ key: e.key, ctrl: e.ctrlKey, shift: e.shiftKey, target: e.target && e.target.id ? e.target.id : e.target ? e.target.tagName : null });
    }, true);
    document.addEventListener('click', (e) => {
      const t = e.target instanceof Element ? e.target.closest('[role="tab"]') : null;
      window.__scInput.clicks.push({ tab: t ? tabName(t) : null, ctrl: e.ctrlKey, target: e.target instanceof Element ? e.target.className || e.target.tagName : null });
    }, true);
    return true;
  };
  const takeInput = () => {
    const log = window.__scInput || { keys: [], clicks: [] };
    window.__scInput = { keys: [], clicks: [] };
    return log;
  };
  // 目前的鍵盤焦點：分頁（tabName）、某分頁的並排鈕（'split:<名稱>'）、關閉鈕（'close:<名稱>'），其餘為 tagName。
  const focusName = () => {
    const a = document.activeElement;
    if (!a) return null;
    if (a.getAttribute('role') === 'tab') return tabName(a);
    const w = a.closest('.review-tab');
    const t = w ? w.querySelector('[role="tab"]') : null;
    if (t && a.classList.contains('review-tab-split')) return `split:${tabName(t)}`;
    if (t && a.classList.contains('review-tab-close')) return `close:${tabName(t)}`;
    return a.tagName;
  };
  // 目前鍵盤焦點所在的區塊（file-split-view task 4.1）：在分頁列內為 'TABBAR'，在某分頁的 tabpanel 內為該分頁的名稱
  // （tabName），其餘為 null。用來記錄 Tab 鍵依序走過哪些欄。
  const focusPanelName = () => {
    const a = document.activeElement;
    if (!a || a === document.body) return null;
    if (reviewTablist() && reviewTablist().contains(a)) return 'TABBAR';
    const p = a.closest('[role="tabpanel"]');
    if (!p) return null;
    const t = reviewTabs().find((x) => panelOf(x) === p);
    return t ? tabName(t) : null;
  };
  // 包裝元素（Live Output 為分頁本身）相對於分頁列可視範圍的位置。
  const tabInView = (name) => {
    const t = tabByName(name);
    const list = reviewTablist();
    if (!t || !list) return null;
    const el = t.parentElement && t.parentElement !== list ? t.parentElement : t;
    const box = list.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    return { left: r.left - box.left, right: box.right - r.right, inView: r.left >= box.left - 1 && r.right <= box.right + 1, scrollLeft: list.scrollLeft, overflow: list.scrollWidth - list.clientWidth };
  };
  window.__sc = {
    txt,
    visible,
    leftTab,
    row,
    rows,
    reviewTabs,
    liveTab,
    fileTab,
    graphTab,
    selectedTab,
    panelOf,
    reviewState,
    stateVersion,
    splitButtonFor,
    splitButtonsIn,
    splitState,
    layoutState,
    reviewChildOrder,
    diffTab,
    tabByName,
    splitUi,
    installInputLog,
    takeInput,
    focusName,
    focusPanelName,
    tabInView,
  };
}

// ---------------------------------------------------------------------------
// 網路記錄（CDP Network 事件；只記主頁面 session，檔案分頁的中繼資料查詢由 files.js 在主頁面發出）
// ---------------------------------------------------------------------------

// `/api/files/<runtime>/<root_id>/meta/<相對路徑>` → 解碼後的相對路徑；不是中繼資料查詢回 null。
function metaPathOf(url) {
  let p;
  try {
    p = new URL(url).pathname;
  } catch {
    return null;
  }
  const m = /^\/api\/files\/[^/]+\/[^/]+\/meta\/(.+)$/.exec(p);
  if (!m) return null;
  try {
    return m[1].split('/').map(decodeURIComponent).join('/');
  } catch {
    return null;
  }
}

// `/api/files/<runtime>/<root_id>/<meta|render|raw>/<相對路徑>` → 解碼後的相對路徑（某個檔案分頁的中繼資料
// 查詢與內容讀取都算；file-split-view task 2.3）；其他端點回 null。
function filePathOf(url) {
  let p;
  try {
    p = new URL(url).pathname;
  } catch {
    return null;
  }
  const m = /^\/api\/files\/[^/]+\/[^/]+\/(?:meta|render|raw)\/(.+)$/.exec(p);
  if (!m) return null;
  try {
    return m[1].split('/').map(decodeURIComponent).join('/');
  } catch {
    return null;
  }
}

function classify(url) {
  let p;
  try {
    p = new URL(url).pathname;
  } catch {
    return 'other';
  }
  if (/^\/api\/runtimes\/[^/]+\/panes\/[^/]+\/output$/.test(p)) return 'output';
  if (/^\/api\/runtimes\/[^/]+\/panes\/[^/]+\/root$/.test(p)) return 'root';
  const m = /^\/api\/files\/[^/]+\/[^/]+\/(list|meta|render|raw)(\/|$)/.exec(p);
  if (m) return m[1];
  if (p === '/api/state') return 'state';
  return 'other';
}

async function recordNetwork(cdp) {
  const reqs = [];
  const byId = new Map();
  cdp.onEvent('Network.requestWillBeSent', (p, sid) => {
    if (sid) return;
    const r = {
      id: p.requestId,
      url: p.request.url,
      kind: classify(p.request.url),
      metaPath: metaPathOf(p.request.url),
      filePath: filePathOf(p.request.url),
      at: Date.now(),
      status: null,
      respondedAt: null,
      failed: false,
      canceled: false,
      failedAt: null,
    };
    byId.set(p.requestId, r);
    reqs.push(r);
  });
  cdp.onEvent('Network.responseReceived', (p, sid) => {
    const r = sid ? null : byId.get(p.requestId);
    if (r) {
      r.status = p.response.status;
      r.respondedAt = Date.now();
    }
  });
  cdp.onEvent('Network.loadingFailed', (p, sid) => {
    const r = sid ? null : byId.get(p.requestId);
    if (r) {
      r.failed = true;
      r.errorText = p.errorText;
      r.canceled = p.canceled === true; // 頁面自己中止（AbortController.abort()）時為 true
      r.failedAt = Date.now();
    }
  });
  await cdp.send('Network.enable');
  return {
    reqs,
    between: (t0, t1, kind) => reqs.filter((r) => r.at >= t0 && r.at <= t1 && (!kind || r.kind === kind)),
  };
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

// ---------------------------------------------------------------------------
// 段落共用：啟動 preview＋chrome、導覽、等首份投影；收尾
// ---------------------------------------------------------------------------

// 「第一份真投影已經畫出」：頂列有 [data-runtime]，且 #version 的 data-state-version 等於 /api/state 的 version。
async function waitForFirstProjection(cdp, previewPort) {
  const start = Date.now();
  while (Date.now() - start < 8000) {
    const s = await cdp
      .eval(`(() => ({ lamp: !!document.querySelector('[data-region="topbar"] [data-runtime]'), v: window.__sc ? window.__sc.stateVersion() : null }))()`)
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

// 目前的投影版本（#version 的 data-state-version；沒有時為 null）。
const stateVersion = (ctx) => ctx.cdp.run(() => window.__sc.stateVersion());
// 等到至少一次整頁重畫（data-state-version 不等於 fromVersion）；回傳新版本，逾時回 null。
async function waitForRepaint(ctx, fromVersion, timeoutMs = UI_TIMEOUT_MS) {
  const v = await ctx.cdp.poll((from) => {
    const now = window.__sc.stateVersion();
    return now !== null && now !== from ? now : false;
  }, [fromVersion], timeoutMs);
  return v === false ? null : v;
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
    ctx.net = await recordNetwork(ctx.cdp);
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

async function stateDump(ctx) {
  return JSON.stringify(await ctx.cdp.run(() => (window.__sc ? window.__sc.reviewState() : { helpers: false })).catch((e) => ({ error: e.message })));
}

// --- 使用者操作（全部經由畫面：真的滑鼠／鍵盤事件）---

async function selectPane(ctx, pane) {
  const sel = `.pane-row[data-runtime="${RUNTIME}"][data-pane="${pane}"]`;
  need(await ctx.cdp.clickEl((s) => document.querySelector(s), [sel], `pane 列 ${RUNTIME}/${pane}`), `點 pane 列 ${RUNTIME}/${pane}`);
  const ok = await ctx.cdp.poll((s) => {
    const r = document.querySelector(s);
    return !!r && r.classList.contains('selected');
  }, [sel], UI_TIMEOUT_MS);
  need(!!ok, `pane 列 ${RUNTIME}/${pane} 出現選定標示`);
}

async function switchLeftTab(ctx, name) {
  need(!!(await ctx.cdp.poll((n) => !!window.__sc.leftTab(n), [name], UI_TIMEOUT_MS)), `找得到左欄分頁「${name}」`);
  need(await ctx.cdp.clickEl((n) => window.__sc.leftTab(n), [name], `左欄分頁「${name}」`), `點左欄分頁「${name}」`);
  const sel = await ctx.cdp.poll((n) => {
    const t = window.__sc.leftTab(n);
    return !!t && t.getAttribute('aria-selected') === 'true';
  }, [name], UI_TIMEOUT_MS);
  need(!!sel, `左欄分頁「${name}」成為目前分頁（aria-selected="true"）`);
}

async function waitRow(ctx, rel) {
  const ok = await ctx.cdp.poll((p) => {
    const r = window.__sc.row(p);
    return !!r && window.__sc.visible(r);
  }, [rel], UI_TIMEOUT_MS);
  need(!!ok, `檔案樹出現可見的列 title="${rel}"`);
}

// 選定 pane（預設 wJ:p4，根目錄 review-repo）並切到左欄「檔案」，等到根目錄第一層出現。
async function openTree(ctx, pane = PANE_REVIEW, probeRow = 'README.md') {
  await selectPane(ctx, pane);
  await switchLeftTab(ctx, '檔案');
  await waitRow(ctx, probeRow);
}

async function expandDir(ctx, rel) {
  await waitRow(ctx, rel);
  const state = await ctx.cdp.run((p) => window.__sc.row(p).getAttribute('aria-expanded'), rel);
  need(state === 'true' || state === 'false', `資料夾列 ${rel} 帶 aria-expanded（實際 ${JSON.stringify(state)}）`);
  if (state === 'true') return;
  need(await ctx.cdp.clickEl((p) => window.__sc.row(p), [rel], `資料夾列 ${rel}`), `點資料夾列 ${rel}`);
  const ok = await ctx.cdp.poll((p) => window.__sc.row(p) && window.__sc.row(p).getAttribute('aria-expanded') === 'true', [rel], UI_TIMEOUT_MS);
  need(!!ok, `資料夾列 ${rel} 展開（aria-expanded="true"）`);
}

// 從檔案樹開檔：點檔案列，等它的檔案分頁出現並成為目前分頁。opts.modifiers 照傳給 clickEl。
async function openFile(ctx, rel, opts = {}) {
  const parts = rel.split('/');
  for (let i = 1; i < parts.length; i++) await expandDir(ctx, parts.slice(0, i).join('/'));
  await waitRow(ctx, rel);
  need(await ctx.cdp.clickEl((p) => window.__sc.row(p), [rel], `檔案列 ${rel}`, opts), `點檔案列 ${rel}`);
  const ok = await ctx.cdp.poll((p) => {
    const t = window.__sc.fileTab(p);
    return !!t && t.getAttribute('aria-selected') === 'true';
  }, [rel], UI_TIMEOUT_MS);
  need(!!ok, `點 ${rel} 後出現並選定它的檔案分頁（data-path="${rel}"、aria-selected="true"）${ok ? '' : `；目前狀態：${await stateDump(ctx)}`}`);
}

// 點分頁列上的分頁（'LIVE'、檔案相對路徑；file-split-view task 3.3 起也接受 'GRAPH' 與 'DIFF:<路徑>'，見
// pageHelpers 的 tabName）。opts.modifiers 照傳給 clickEl；opts.expectSelected＝false 時不等它成為目前分頁
// （給 Ctrl＋點選等不一定改目前分頁的操作用）。
async function clickTab(ctx, which, opts = {}) {
  const finder = (n) => window.__sc.tabByName(n);
  const args = [which];
  const desc = which === 'LIVE' ? 'Live Output 分頁' : which === 'GRAPH' ? 'Git Graph 分頁' : `分頁 ${which}`;
  need(!!(await ctx.cdp.poll(finder, args, UI_TIMEOUT_MS)), `找得到${desc}`);
  need(await ctx.cdp.clickEl(finder, args, desc, opts), `點${desc}${opts.modifiers ? `（modifiers=${opts.modifiers}）` : ''}`);
  if (opts.expectSelected === false) return;
  const sel = await ctx.cdp.poll((n) => !!window.__sc.tabByName(n) && window.__sc.tabByName(n).getAttribute('aria-selected') === 'true', args, UI_TIMEOUT_MS);
  need(!!sel, `${desc}成為目前分頁（aria-selected="true"）`);
}

const reviewState = (ctx) => ctx.cdp.run(() => window.__sc.reviewState());

// 斷言：分頁區實際可見的 tabpanel 恰為 expected（依分頁列順序的名稱陣列，'LIVE' 或相對路徑），
// 其餘 tabpanel 都設 hidden 且不可見；沒有無主的 tabpanel 可見。
function checkVisiblePanels(state, expected, label) {
  const shown = state.tabs.filter((t) => t.visible).map((t) => t.name);
  check(JSON.stringify(shown) === JSON.stringify(expected), `${label}：實際可見的分頁內容恰為 ${JSON.stringify(expected)}（實際 ${JSON.stringify(shown)}）`);
  const leaked = state.tabs.filter((t) => !expected.includes(t.name) && (t.hidden !== true || t.visible)).map((t) => `${t.name}(hidden=${t.hidden},visible=${t.visible})`);
  check(leaked.length === 0, `${label}：其餘分頁的 tabpanel 都設 hidden 且不可見（違反：${JSON.stringify(leaked)}）`);
  const orphanShown = state.orphans.filter((o) => o.visible).map((o) => o.id);
  check(orphanShown.length === 0, `${label}：#review 內沒有無主的可見 tabpanel（實際 ${JSON.stringify(orphanShown)}）`);
}

// 觀察 windowMs 毫秒內服務收到的中繼資料查詢（CDP Network；主頁面發出的才算），回傳依相對路徑分組的次數。
// 開始觀察前先等 settleMs，讓切換前已發出、正被中止的請求不落在窗內。
async function observeMeta(ctx, windowMs, settleMs = 500) {
  await sleep(settleMs);
  const t0 = Date.now();
  await sleep(windowMs);
  const t1 = Date.now();
  const reqs = ctx.net.between(t0, t1, 'meta');
  const byPath = {};
  for (const r of reqs) byPath[r.metaPath] = (byPath[r.metaPath] || 0) + 1;
  return { byPath, total: reqs.length, windowMs: t1 - t0 };
}

// 以 CDP Fetch 在 Request 階段攔住某個檔案的中繼資料查詢（file-split-view task 2.3）：查詢送不到服務，頁面
// 一直等著，直到 release() 以指定的回應放行。用來模擬「回應很慢」，確保切換或關閉一定發生在回應回來之前。
// held 每筆為 { requestId, networkId, at, outcome, error, releasedAt }。release() 之後 outcome 是 'delivered'
// （放行成功，頁面收得到這份回應）或 'gone'（Chrome 回報這筆已不存在，通常是頁面已經中止它）。
async function installMetaHold(ctx, relPath) {
  const suffix = `/meta/${relPath.split('/').map(encodeURIComponent).join('/')}`;
  const held = [];
  ctx.cdp.onEvent('Fetch.requestPaused', (p, sid) => {
    if (sid) return;
    held.push({ requestId: p.requestId, networkId: p.networkId, at: Date.now(), outcome: null, error: null, releasedAt: null });
  });
  const r = await ctx.cdp.send('Fetch.enable', { patterns: [{ urlPattern: `*${suffix}`, requestStage: 'Request' }] });
  need(!r.error, `Fetch.enable 攔截 *${suffix}（${r.error ? JSON.stringify(r.error) : 'ok'}）`);
  return {
    held,
    // 以 HTTP status＋JSON body 放行所有還沒處理過的攔截。
    async release(status, body) {
      for (const h of held) {
        if (h.outcome !== null) continue;
        const res = await ctx.cdp.send('Fetch.fulfillRequest', {
          requestId: h.requestId,
          responseCode: status,
          responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
          body: Buffer.from(JSON.stringify(body), 'utf8').toString('base64'),
        });
        h.outcome = res.error ? 'gone' : 'delivered';
        h.error = res.error ? res.error.message : null;
        h.releasedAt = Date.now();
      }
    },
  };
}

// 等到 installMetaHold() 攔下第一筆（目前分頁每 2 秒查一次，最多等一個輪詢間隔加 UI 逾時）。
async function waitFirstHeld(hold, rel) {
  const start = Date.now();
  while (hold.held.length === 0 && Date.now() - start < POLL_INTERVAL_MS + UI_TIMEOUT_MS) await sleep(50);
  need(hold.held.length > 0, `${rel} 有一筆中繼資料查詢卡在半路（被 Fetch 攔下）`);
}

// 某個檔案分頁的狀態快照：分頁本身與它的 tabpanel（面板 hidden 時也讀得到）。分頁不存在時回 null。
const fileSnap = (ctx, rel) =>
  ctx.cdp.run((p) => {
    const t = window.__sc.fileTab(p);
    const panel = window.__sc.panelOf(t);
    if (!t || !panel) return null;
    const st = panel.querySelector('.file-status');
    const host = panel.querySelector('.file-viewer-host');
    const icon = t.querySelector('.review-tab-icon');
    return {
      selected: t.getAttribute('aria-selected') === 'true',
      stale: panel.classList.contains('is-stale'),
      statusShown: !!st && !st.hidden,
      statusText: st ? st.textContent : null,
      icon: icon ? icon.getAttribute('src') : null,
      content: host ? host.textContent : '',
    };
  }, rel);

// 等到某個檔案分頁的內容畫好：狀態列隱藏（不是讀取中也不是失敗）、沒有過期標示、內容非空。
async function waitFileReady(ctx, rel) {
  const ok = await ctx.cdp.poll((p) => {
    const panel = window.__sc.panelOf(window.__sc.fileTab(p));
    if (!panel) return false;
    const st = panel.querySelector('.file-status');
    const host = panel.querySelector('.file-viewer-host');
    return !panel.classList.contains('is-stale') && !!st && st.hidden && !!host && host.textContent.trim() !== '';
  }, [rel], UI_TIMEOUT_MS);
  need(!!ok, `${rel} 的內容已畫好（狀態列隱藏、沒有過期標示、內容非空）${ok ? '' : `；快照：${JSON.stringify(await fileSnap(ctx, rel))}`}`);
}

// 某個檔案在 t0 之後發出的中繼資料查詢與內容讀取（meta／render／raw）；excludeIds 裡的 requestId 不算。
const fileReqsSince = (ctx, rel, t0, excludeIds = []) => ctx.net.reqs.filter((r) => r.filePath === rel && r.at > t0 && !excludeIds.includes(r.id));

// 等到某一筆請求被頁面中止（Network.loadingFailed 且 canceled）；回傳該筆的記錄，逾時回 null。
async function waitCanceled(ctx, networkId, timeoutMs) {
  const start = Date.now();
  for (;;) {
    const r = ctx.net.reqs.find((x) => x.id === networkId);
    if (r && r.failed) return r;
    if (Date.now() - start >= timeoutMs) return r && r.failed ? r : null;
    await sleep(50);
  }
}

// ---------------------------------------------------------------------------
// 段落
// ---------------------------------------------------------------------------

// 純函式自我測試：中繼資料網址解析（含百分比編碼、巢狀路徑、非中繼資料端點）與請求分類。
async function segSelfScaffold() {
  const o = 'http://127.0.0.1:7910';
  const cases = [
    [`${o}/api/files/win/r1/meta/README.md`, 'README.md'],
    [`${o}/api/files/win/r1/meta/docs/a.md`, 'docs/a.md'],
    [`${o}/api/files/win/r1/meta/docs/a%20b.md`, 'docs/a b.md'],
    [`${o}/api/files/win/r1/meta/%E4%B8%AD%E6%96%87.md?x=1`, '中文.md'],
    [`${o}/api/files/win/r1/render/docs/a.md`, null],
    [`${o}/api/files/win/r1/list/docs`, null],
    [`${o}/api/state`, null],
    ['not a url', null],
  ];
  for (const [url, want] of cases) {
    const got = metaPathOf(url);
    check(got === want, `metaPathOf(${JSON.stringify(url)}) === ${JSON.stringify(want)}（實際 ${JSON.stringify(got)}）`);
  }
  const fileCases = [
    [`${o}/api/files/win/r1/meta/docs/a.md`, 'docs/a.md'],
    [`${o}/api/files/win/r1/render/docs/a.md`, 'docs/a.md'],
    [`${o}/api/files/win/r1/raw/report.pdf`, 'report.pdf'],
    [`${o}/api/files/win/r1/raw/docs/a%20b.md`, 'docs/a b.md'],
    [`${o}/api/files/win/r1/list/docs`, null],
    [`${o}/api/files/win/r1/meta`, null],
    [`${o}/api/state`, null],
    ['not a url', null],
  ];
  for (const [url, want] of fileCases) {
    const got = filePathOf(url);
    check(got === want, `filePathOf(${JSON.stringify(url)}) === ${JSON.stringify(want)}（實際 ${JSON.stringify(got)}）`);
  }
  const kinds = [
    [`${o}/api/files/win/r1/meta/a.md`, 'meta'],
    [`${o}/api/files/win/r1/render/a.md`, 'render'],
    [`${o}/api/files/win/r1/raw/a.pdf`, 'raw'],
    [`${o}/api/files/win/r1/list`, 'list'],
    [`${o}/api/runtimes/win/panes/wJ%3Ap4/root`, 'root'],
    [`${o}/api/runtimes/win/panes/wJ%3Ap4/output`, 'output'],
    [`${o}/api/state`, 'state'],
    [`${o}/app/files.js`, 'other'],
  ];
  for (const [url, want] of kinds) {
    const got = classify(url);
    check(got === want, `classify(${JSON.stringify(url)}) === ${JSON.stringify(want)}（實際 ${JSON.stringify(got)}）`);
  }
}

// parseSegmentArg 的合法與不合法輸入；再實際以拼錯的代號與空字串執行本檔：必須 exit 2、印
// `RESULT: FAIL (段落代號)`，而且不啟動任何行程（參數檢查在環境檢查與啟動之前）。
async function segSelfSegmentArg() {
  const known = ['self/a', 'self/b', 'file-review/x y'];
  const cases = [
    [undefined, true, null],
    ['self/a', true, ['self/a']],
    ['self/', true, ['self/a', 'self/b']],
    ['self/a, file-review/x y', true, ['self/a', 'file-review/x y']],
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

// 基準（file-split-view task 2.1；spec file-review「自動更新」：只有可見的檔案分頁被查詢；沒有並排時可見的
// 就是目前分頁）：開 README.md 與 docs/a.md 兩個檔案分頁，
//   - 實際可見的分頁內容只有目前分頁（docs/a.md），README.md 與 Live Output 的 tabpanel 都 hidden；
//   - 觀察 6 秒：服務收到的中繼資料查詢全是 docs/a.md（至少 2 次），README.md 0 次；
//   - 點 README.md 分頁後反過來：只有 README.md 可見；觀察 6 秒只有 README.md 被查詢。
//   - 沒有並排時 #review 沒有 data-split 屬性（design D4）。
async function segBaselineOnlyCurrentVisible() {
  await withCockpit('baseline', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    await openFile(ctx, 'docs/a.md');
    // 門檻（task 2.1 實跑：兩個方向各 6 秒窗內都是 3 次）：相鄰兩次查詢的開始至少相隔 2 秒加一次回應時間，
    // 6 秒窗理論上 2～3 次，看窗與輪詢相位怎麼對齊。要求 3 次會在邊界上偶發失敗，所以取 2；回應時間在 1 秒以內
    // 時至少 2 次是保證的。這條只防「目前分頁沒在輪詢」，主斷言是另一個分頁 0 次。
    const windowMs = 3 * POLL_INTERVAL_MS;
    const minHits = 2;
    for (const [current, other] of [
      ['docs/a.md', 'README.md'],
      ['README.md', 'docs/a.md'],
    ]) {
      if (current === 'README.md') await clickTab(ctx, 'README.md');
      const state = await reviewState(ctx);
      check(state.selected === current, `目前分頁是 ${current}（實際 ${JSON.stringify(state.selected)}）`);
      check(state.tabs.filter((t) => t.selected).length === 1, `恰有一個分頁 aria-selected="true"（實際 ${state.tabs.filter((t) => t.selected).length}）`);
      check(state.split === null, `沒有並排時 #review 沒有 data-split 屬性（實際 ${JSON.stringify(state.split)}）`);
      checkVisiblePanels(state, [current], `目前分頁為 ${current}`);
      const seen = await observeMeta(ctx, windowMs);
      log(`目前分頁 ${current}：${seen.windowMs} ms 內的中繼資料查詢 ${JSON.stringify(seen.byPath)}`);
      check((seen.byPath[current] || 0) >= minHits, `目前分頁 ${current} 在 ${windowMs} ms 內被查詢至少 ${minHits} 次（實際 ${seen.byPath[current] || 0}）`);
      check((seen.byPath[other] || 0) === 0, `不可見的 ${other} 沒有被查詢（實際 ${seen.byPath[other] || 0} 次）`);
      const strays = Object.keys(seen.byPath).filter((p) => p !== current);
      check(strays.length === 0, `窗內只有 ${current} 的中繼資料查詢（其他路徑：${JSON.stringify(strays)}）`);
    }
    const exceptions = ctx.console.filter((e) => e.level === 'exception');
    check(exceptions.length === 0, `頁面沒有未捕捉例外（實際 ${JSON.stringify(exceptions.map((e) => e.text))}）`);
  });
}

// 放行攔下的查詢時用的回應：若被套用，該分頁會顯示「檔案已不存在」並標為過期（畫面上有內容時），很容易看出來。
const STALE_RESPONSE = { status: 404, body: { code: 'not_found', message: 'split-check：攔下的舊回應' } };

// 開 README.md 與 docs/a.md 兩個檔案分頁並等兩者內容都畫好；結束時目前分頁是 docs/a.md。
async function openTwoReadyTabs(ctx) {
  await openTree(ctx);
  await openFile(ctx, 'README.md');
  await waitFileReady(ctx, 'README.md');
  await openFile(ctx, 'docs/a.md');
  await waitFileReady(ctx, 'docs/a.md');
}

// file-split-view task 2.3（spec file-review「自動更新」：分頁變成不可見之後才回來的舊回應必須丟棄；design D3：
// 每個分頁各自用 gen 丟棄舊回應，deactivate 時中止進行中的請求）。A＝docs/a.md，B＝README.md。
//   前置：兩者內容都畫好，目前為 A；以 Fetch 攔住 A 的中繼資料查詢，等到 A 有一筆卡在半路。
//   操作：不等選定，連點 B → A → B（clickTab 的 expectSelected:false），等 B 成為目前分頁、B 自己的查詢回來一次後，
//   把所有攔下的 A 查詢以 404 not_found 放行（若被套用，畫面會出現「檔案已不存在」與過期標示）。
//   斷言（放行後再等 2.5 秒，跨過 B 的一次輪詢）：
//     - 自我驗證：攔下的 A 查詢恰 2 筆（切走前那筆＋中間那下讓 A 變成可見時立即查的那筆），證明中間那下點擊確實
//       讓 A 變成可見，連點沒有被吃掉。
//     - B：仍為目前分頁；沒有過期標示、狀態列隱藏；分頁圖示與內容和切換前相同。
//     - A（不可見）：沒有過期標示、狀態列隱藏，舊回應沒有套用到 A。
//     - 最後一下之後，A 沒有任何新的中繼資料查詢或內容讀取（render／raw）。
//     - 每筆攔下的 A 查詢都在放行前就被頁面中止（Network.loadingFailed canceled），放行時 Chrome 回報已不存在。
//     - 頁面沒有未捕捉例外。
async function segRapidSwitchDiscardsStale() {
  const A = 'docs/a.md';
  const B = 'README.md';
  await withCockpit('rapid', {}, async (ctx) => {
    await openTwoReadyTabs(ctx);
    const bBefore = await fileSnap(ctx, B);
    need(!!bBefore && bBefore.content.trim() !== '', `切換前記下 ${B} 的快照（${JSON.stringify(bBefore && { icon: bBefore.icon, len: bBefore.content.length })}）`);
    const hold = await installMetaHold(ctx, A);
    await waitFirstHeld(hold, A);

    await clickTab(ctx, B, { expectSelected: false });
    await clickTab(ctx, A, { expectSelected: false });
    await clickTab(ctx, B, { expectSelected: false });
    const tFinal = Date.now();
    const heldIdsAtFinal = hold.held.map((h) => h.networkId);
    const selB = await ctx.cdp.poll((p) => !!window.__sc.fileTab(p) && window.__sc.fileTab(p).getAttribute('aria-selected') === 'true', [B], UI_TIMEOUT_MS);
    need(!!selB, `連點 B → A → B 之後 ${B} 成為目前分頁${selB ? '' : `；目前狀態：${await stateDump(ctx)}`}`);
    need(hold.held.length === 2, `自我驗證：攔下的 ${A} 查詢恰 2 筆（切走前 1 筆＋中間那下點擊讓 ${A} 可見時 1 筆；實際 ${hold.held.length}）`);
    // 最後一下點擊的處理比 Input.dispatchMouseEvent 的回覆晚幾毫秒（task 2.3 實測：中止事件落在回覆後 2～16 ms），
    // 所以不能拿「點完」當作切換已完成。改等 B 在中間那下（A 第 2 筆被攔下）之後發出的查詢回來：B 在那之後才再次
    // 變成可見，這筆一定是最後一下觸發的，它回來時最後一下的處理（先 deactivate A、再 activate B）早已跑完。再多等
    // 300 ms 讓中止事件送達。
    const tSecondHeld = hold.held[1].at;
    const bAnswered = await (async () => {
      const start = Date.now();
      while (Date.now() - start < UI_TIMEOUT_MS) {
        if (ctx.net.reqs.some((r) => r.metaPath === B && r.at > tSecondHeld && r.respondedAt !== null)) return true;
        await sleep(50);
      }
      return false;
    })();
    need(bAnswered, `${B} 在最後一下點擊後變成可見時發出的中繼資料查詢已回來`);
    await sleep(300);

    await hold.release(STALE_RESPONSE.status, STALE_RESPONSE.body);
    const tRelease = Date.now();
    await sleep(POLL_INTERVAL_MS + 500);

    const bAfter = await fileSnap(ctx, B);
    const aAfter = await fileSnap(ctx, A);
    log(`放行後 ${B}：${JSON.stringify(bAfter && { ...bAfter, content: bAfter.content.length })}；${A}：${JSON.stringify(aAfter && { ...aAfter, content: aAfter.content.length })}`);
    need(!!bAfter && !!aAfter, `放行後兩個分頁都還在`);
    check(bAfter.selected, `${B} 仍為目前分頁`);
    check(!bAfter.stale, `${B} 沒有過期標示（舊回應沒有蓋到 B）`);
    check(!bAfter.statusShown, `${B} 的狀態列隱藏（實際顯示 ${JSON.stringify(bAfter.statusText)}）`);
    check(bAfter.icon === bBefore.icon, `${B} 的分頁圖示和切換前相同（前 ${bBefore.icon}，後 ${bAfter.icon}）`);
    check(bAfter.content === bBefore.content, `${B} 的內容和切換前相同（長度 前 ${bBefore.content.length}、後 ${bAfter.content.length}）`);
    check(!aAfter.stale, `${A}（不可見）沒有過期標示（舊回應沒有套用到 A）`);
    check(!aAfter.statusShown, `${A}（不可見）的狀態列隱藏（實際顯示 ${JSON.stringify(aAfter.statusText)}）`);
    const late = fileReqsSince(ctx, A, tFinal, heldIdsAtFinal);
    check(late.length === 0, `最後一下之後 ${A} 沒有新的中繼資料查詢或內容讀取（實際 ${JSON.stringify(late.map((r) => `${r.kind}@+${r.at - tFinal}ms`))}）`);
    hold.held.forEach((h, i) => {
      const r = ctx.net.reqs.find((x) => x.id === h.networkId);
      const abortedBeforeRelease = !!r && r.failed && r.canceled && r.failedAt !== null && r.failedAt <= (h.releasedAt || tRelease);
      const rel = (t) => (t === null || t === undefined ? null : t - tFinal);
      check(
        abortedBeforeRelease,
        `攔下的第 ${i + 1} 筆 ${A} 查詢在放行前就被頁面中止（Network.loadingFailed canceled；實際 ${JSON.stringify(
          r ? { failed: r.failed, canceled: r.canceled, errorText: r.errorText, status: r.status, heldAt: rel(h.at), failedAt: rel(r.failedAt), releasedAt: rel(h.releasedAt) } : null
        )}；時間為相對最後一下點擊的毫秒）`
      );
      check(h.outcome === 'gone', `攔下的第 ${i + 1} 筆 ${A} 查詢放行時 Chrome 回報已不存在（實際 ${h.outcome}${h.error ? `：${h.error}` : ''}）`);
    });
    const exceptions = ctx.console.filter((e) => e.level === 'exception');
    check(exceptions.length === 0, `頁面沒有未捕捉例外（實際 ${JSON.stringify(exceptions.map((e) => e.text))}）`);
  });
}

// file-split-view task 2.3（spec file-review「自動更新」：分頁被關閉之後才回來的舊回應必須丟棄；design D3：
// 關閉時中止進行中的請求）。
//   前置：開 README.md 與 docs/a.md，內容都畫好，目前為 docs/a.md；以 Fetch 攔住 docs/a.md 的中繼資料查詢，等到
//   一筆卡在半路。
//   操作：點 docs/a.md 的關閉鈕。
//   斷言：docs/a.md 的分頁與 tabpanel 都移除、README.md 成為目前分頁（關閉目前分頁、右側沒有分頁→左側）；卡住的
//   那筆在 1 秒內被頁面中止（canceled）；之後放行時 Chrome 回報已不存在；關閉後觀察 2.5 秒，docs/a.md 沒有新的查詢
//   或讀取；README.md 沒有過期標示；頁面沒有未捕捉例外。
async function segCloseAbortsInflight() {
  const A = 'docs/a.md';
  const B = 'README.md';
  await withCockpit('close', {}, async (ctx) => {
    await openTwoReadyTabs(ctx);
    const hold = await installMetaHold(ctx, A);
    await waitFirstHeld(hold, A);
    const panelId = await ctx.cdp.run((p) => window.__sc.fileTab(p).getAttribute('aria-controls'), A);

    need(
      await ctx.cdp.clickEl((p) => {
        const t = window.__sc.fileTab(p);
        return t && t.parentElement ? t.parentElement.querySelector('.review-tab-close') : null;
      }, [A], `${A} 的關閉鈕`),
      `點 ${A} 的關閉鈕`
    );
    const tClose = Date.now();
    const heldIdsAtClose = hold.held.map((h) => h.networkId);
    const gone = await ctx.cdp.poll((p, id) => !window.__sc.fileTab(p) && !document.getElementById(id), [A, panelId], UI_TIMEOUT_MS);
    need(!!gone, `${A} 的分頁與 tabpanel（#${panelId}）都已移除`);
    const state = await reviewState(ctx);
    check(state.selected === B, `關閉後 ${B} 成為目前分頁（實際 ${JSON.stringify(state.selected)}）`);

    const r = await waitCanceled(ctx, hold.held[0].networkId, 1000);
    check(!!r && r.canceled, `關閉後 1 秒內，卡住的 ${A} 查詢被頁面中止（Network.loadingFailed canceled；實際 ${JSON.stringify(r ? { canceled: r.canceled, errorText: r.errorText, after: r.failedAt - tClose } : null)}）`);
    await hold.release(STALE_RESPONSE.status, STALE_RESPONSE.body);
    hold.held.forEach((h, i) => {
      check(h.outcome === 'gone', `攔下的第 ${i + 1} 筆 ${A} 查詢放行時 Chrome 回報已不存在（實際 ${h.outcome}${h.error ? `：${h.error}` : ''}）`);
    });
    await sleep(POLL_INTERVAL_MS + 500);
    const late = fileReqsSince(ctx, A, tClose, heldIdsAtClose);
    check(late.length === 0, `關閉後 ${A} 沒有新的中繼資料查詢或內容讀取（實際 ${JSON.stringify(late.map((x) => `${x.kind}@+${x.at - tClose}ms`))}）`);
    const b = await fileSnap(ctx, B);
    check(!!b && !b.stale && !b.statusShown, `${B} 沒有過期標示、狀態列隱藏（實際 ${JSON.stringify(b && { stale: b.stale, status: b.statusShown ? b.statusText : null })}）`);
    const exceptions = ctx.console.filter((e) => e.level === 'exception');
    check(exceptions.length === 0, `頁面沒有未捕捉例外（實際 ${JSON.stringify(exceptions.map((e) => e.text))}）`);
  });
}

// ---------------------------------------------------------------------------
// 並排的狀態轉換（file-split-view task 3.1；spec file-review「檔案並排」「自動更新」；design D1、D2、D7、D9）
//
// 3.1 只驗狀態，不驗幾何（控制端裁決 A）：哪些 tabpanel 實際可見、欄位順序（包裝元素 .review-tab 的
// data-split-col）、目前分頁（aria-selected）、並排鈕的 aria-pressed、其他欄沒有重新讀取內容、捲動位置不變。
// 等寬與版面是 3.2 的事。spec scenario 的檔名對應到 review-repo fixture：a.md→README.md、b.md→docs/a.md、
// c.md→long.md、d.md→note.txt；需要捲動的欄用夠長的 long.md 與 docs/design.md（各段開頭註明）。
// ---------------------------------------------------------------------------

const splitState = (ctx) => ctx.cdp.run(() => window.__sc.splitState());

function noExceptions(ctx) {
  const exceptions = ctx.console.filter((e) => e.level === 'exception');
  check(exceptions.length === 0, `頁面沒有未捕捉例外（實際 ${JSON.stringify(exceptions.map((e) => e.text))}）`);
}

// 從檔案樹開檔並等內容畫好（之後才切走，避免讀到一半被作廢、切回時才重讀，干擾「沒有重新讀取」的斷言）。
async function openReady(ctx, rel) {
  await openFile(ctx, rel);
  await waitFileReady(ctx, rel);
}

// 按某個檔案分頁的並排鈕（真的滑鼠事件）。
async function pressSplit(ctx, rel) {
  const has = await ctx.cdp.poll((p) => !!window.__sc.splitButtonFor(window.__sc.fileTab(p)), [rel], UI_TIMEOUT_MS);
  need(!!has, `${rel} 的分頁上有並排鈕（.review-tab 內帶 aria-pressed 的 <button>）${has ? '' : `；狀態：${JSON.stringify(await splitState(ctx))}`}`);
  need(await ctx.cdp.clickEl((p) => window.__sc.splitButtonFor(window.__sc.fileTab(p)), [rel], `${rel} 的並排鈕`), `點 ${rel} 的並排鈕`);
}

// 按某個檔案分頁的關閉鈕；回傳按下的時間。
async function pressClose(ctx, rel) {
  need(
    await ctx.cdp.clickEl((p) => {
      const t = window.__sc.fileTab(p);
      return t && t.parentElement ? t.parentElement.querySelector('.review-tab-close') : null;
    }, [rel], `${rel} 的關閉鈕`),
    `點 ${rel} 的關閉鈕`
  );
  return Date.now();
}

// --- file-split-view task 3.3 的共用工具 ---

// 從左欄「變更」的「Git Graph」按鈕打開 Git Graph 分頁，等它成為目前分頁。
async function openGitGraph(ctx) {
  await switchLeftTab(ctx, '變更');
  const btnSel = '[data-action="open-git-graph"]';
  const btn = await ctx.cdp.poll((s) => {
    const b = document.querySelector(s);
    return !!b && window.__sc.visible(b);
  }, [btnSel], UI_TIMEOUT_MS);
  need(!!btn, '左欄「變更」的「Git Graph」按鈕出現且可見');
  need(await ctx.cdp.clickEl((s) => document.querySelector(s), [btnSel], '「Git Graph」按鈕'), '點「Git Graph」按鈕');
  const sel = await ctx.cdp.poll(() => !!window.__sc.graphTab() && window.__sc.graphTab().getAttribute('aria-selected') === 'true', [], UI_TIMEOUT_MS);
  need(!!sel, 'Git Graph 分頁成為目前分頁');
}

const splitUi = (ctx) => ctx.cdp.run(() => window.__sc.splitUi());

// 並排鈕相關的介面文字（i18n.js 字典；lang 為 'zh' 或 'en'）。先確認四個鍵在兩份字典都存在，再回傳該語言的值。
// split：並排鈕可用時的 title；splitDisabled：停用時的 title（說明原因）；splitNamed：aria-label 範本（{name}）；
// splitCol：「並排第 N 欄」說明的範本（{n}）；title：分頁 title 的範本（{path}、{root}；修正第 1 輪起也是並排成員的第二段說明）。
const SPLIT_TEXT_KEYS = { split: 'files.tab.split', splitDisabled: 'files.tab.splitDisabled', splitNamed: 'files.tab.splitNamed', splitCol: 'files.tab.splitCol', title: 'files.tab.title' };
// ui_preview fixture 的根目錄名稱（pane wJ:p4 的根目錄；分頁 title 的 {root}）。
const ROOT_NAME = 'review-repo';
async function dictTexts(ctx, lang) {
  const d = await ctx.cdp.run((keys, lg) => {
    const dicts = window.cockpitI18n && window.cockpitI18n.dictionaries;
    if (!dicts) return null;
    const out = { missing: [] };
    for (const [k, key] of Object.entries(keys)) {
      for (const l of ['zh', 'en']) if (typeof dicts[l][key] !== 'string') out.missing.push(`${l}:${key}`);
      out[k] = dicts[lg][key];
    }
    return out;
  }, SPLIT_TEXT_KEYS, lang);
  need(!!d && d.missing.length === 0, `i18n.js 中英兩份字典都有並排鈕相關的鍵 ${JSON.stringify(Object.values(SPLIT_TEXT_KEYS))}（缺 ${JSON.stringify(d ? d.missing : 'window.cockpitI18n')}）`);
  return d;
}
const fill = (tpl, params) => tpl.replace(/\{(\w+)\}/g, (m, k) => (params[k] !== undefined ? String(params[k]) : m));

// 斷言 rel 的並排鈕為停用狀態（spec「加入」第二點；design D7）：aria-disabled="true"、不帶 HTML 的 disabled 屬性
// （滑鼠移上去才看得到 title）、title 為停用原因（字典值，且說明要先選另一個檔案分頁）、aria-pressed="false"。
async function expectDisabled(ctx, rel, texts, label) {
  const u = (await splitUi(ctx))[rel];
  need(!!u && u.buttons === 1, `${label}：${rel} 的分頁上恰有一個並排鈕（實際 ${JSON.stringify(u)}）`);
  check(u.ariaDisabled === 'true', `${label}：${rel} 的並排鈕 aria-disabled="true"（實際 ${JSON.stringify(u.ariaDisabled)}）`);
  check(u.disabledAttr === false, `${label}：${rel} 的並排鈕不帶 disabled 屬性（用 aria-disabled，title 才看得到）`);
  check(u.title === texts.splitDisabled, `${label}：${rel} 的並排鈕 title 為停用原因 ${JSON.stringify(texts.splitDisabled)}（實際 ${JSON.stringify(u.title)}）`);
  check(u.pressed === 'false', `${label}：${rel} 的並排鈕 aria-pressed="false"（實際 ${JSON.stringify(u.pressed)}）`);
}

// 斷言 rel 的並排鈕可用：沒有 aria-disabled="true"、title 為「並排」。
async function expectEnabled(ctx, rel, texts, label) {
  const u = (await splitUi(ctx))[rel];
  need(!!u && u.buttons === 1, `${label}：${rel} 的分頁上恰有一個並排鈕（實際 ${JSON.stringify(u)}）`);
  check(u.ariaDisabled !== 'true', `${label}：${rel} 的並排鈕可用（aria-disabled 實際 ${JSON.stringify(u.ariaDisabled)}）`);
  check(u.disabledAttr === false, `${label}：${rel} 的並排鈕不帶 disabled 屬性`);
  check(u.title === texts.split, `${label}：${rel} 的並排鈕 title 為 ${JSON.stringify(texts.split)}（實際 ${JSON.stringify(u.title)}）`);
}

// 滑鼠移到 (x, y)（真的 mouseMoved 事件，:hover 跟著變）。
async function mouseTo(ctx, x, y) {
  await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
  await sleep(120);
}
// 滑鼠移到分頁列以外（頁面左上角 (2, 2)，先確認那裡不在分頁列裡）。
async function mouseAway(ctx) {
  const outside = await ctx.cdp.run(() => {
    const h = document.elementFromPoint(2, 2);
    const list = document.querySelector('#review [role="tablist"]');
    return !!h && !!list && !list.contains(h);
  });
  need(outside, '頁面 (2, 2) 不在分頁區的分頁列裡（滑鼠移開用）');
  await mouseTo(ctx, 2, 2);
}
// 滑鼠移到某個分頁本身的中心（不按）。
async function hoverTab(ctx, name) {
  const pt = await ctx.cdp.run((n) => {
    const t = window.__sc.tabByName(n);
    if (!t) return null;
    t.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    const r = t.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  }, name);
  need(!!pt, `找得到分頁 ${name}（滑鼠移上去用）`);
  await mouseTo(ctx, pt.x, pt.y);
}

// 無障礙樹上某個元素（CSS 選擇器）的 role、計算後的名稱與說明（CDP Accessibility.getPartialAXTree）。
async function axOf(ctx, selector) {
  const doc = await ctx.cdp.send('DOM.getDocument', { depth: 0 });
  if (!doc.result) return { error: JSON.stringify(doc.error) };
  const q = await ctx.cdp.send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector });
  if (!q.result || !q.result.nodeId) return { error: `找不到 ${selector}` };
  const ax = await ctx.cdp.send('Accessibility.getPartialAXTree', { nodeId: q.result.nodeId, fetchRelatives: false });
  const n = ax.result && ax.result.nodes ? ax.result.nodes[0] : null;
  if (!n) return { error: JSON.stringify(ax.error || ax.result) };
  return { role: n.role ? n.role.value : null, name: n.name ? n.name.value : '', description: n.description ? n.description.value : '' };
}
const tabSelector = async (ctx, name) => {
  const id = await ctx.cdp.run((n) => {
    const t = window.__sc.tabByName(n);
    return t ? t.id : null;
  }, name);
  need(!!id, `分頁 ${name} 有 id`);
  return `#${id}`;
};

// 斷言並排狀態：exp = { cols: 並排組合依欄位順序（沒有並排組合為 []）, selected: 目前分頁, shown: 實際可見的分頁（不計順序） }。
// 先輪詢到狀態相符（或逾時），再逐項 check，失敗訊息附實際值。另驗：欄位編號連續 1～n、恰一個 aria-selected、
// 每個並排鈕的 aria-pressed 恰好反映是否在並排組合中、其餘 tabpanel 都 hidden 且不可見。回傳最後的快照。
async function expectSplit(ctx, exp, label) {
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const sorted = (a) => [...a].sort();
  const matches = (s) => !!s && same(s.cols, exp.cols) && s.selected === exp.selected && same(sorted(s.shown), sorted(exp.shown));
  let s = null;
  const start = Date.now();
  for (;;) {
    s = await splitState(ctx).catch(() => null);
    if (matches(s) || Date.now() - start >= UI_TIMEOUT_MS) break;
    await sleep(100);
  }
  need(!!s, `${label}：讀得到分頁區狀態`);
  check(same(s.cols, exp.cols), `${label}：並排組合依序為 ${JSON.stringify(exp.cols)}（data-split-col；實際 ${JSON.stringify(s.cols)}）`);
  check(same(s.colNums, exp.cols.map((_, i) => i + 1)), `${label}：欄位編號為連續的 1～${exp.cols.length}（實際 ${JSON.stringify(s.colNums)}）`);
  check(s.selected === exp.selected, `${label}：目前分頁（aria-selected="true"）為 ${exp.selected}（實際 ${JSON.stringify(s.selected)}）`);
  check(s.selectedCount === 1, `${label}：恰有一個分頁 aria-selected="true"（實際 ${s.selectedCount}）`);
  const wrongPressed = Object.entries(s.pressed)
    .filter(([name, v]) => v !== (exp.cols.includes(name) ? 'true' : 'false'))
    .map(([name, v]) => `${name}=${v}`);
  check(wrongPressed.length === 0, `${label}：並排組合中的分頁並排鈕 aria-pressed="true"、其餘 "false"（不符：${JSON.stringify(wrongPressed)}；全部 ${JSON.stringify(s.pressed)}）`);
  const missing = exp.shown.filter((n) => !s.names.includes(n));
  check(missing.length === 0, `${label}：預期可見的分頁都在分頁列上（缺 ${JSON.stringify(missing)}）`);
  checkVisiblePanels(await reviewState(ctx), s.names.filter((n) => exp.shown.includes(n)), label);
  await checkLayout(ctx, exp, label);
  return s;
}

// 並排版面（file-split-view task 3.2；design D4、裁決 C；spec「檔案並排」的「版面」與「加入並排」的「等寬兩欄」）：
//   - 並排顯示中（exp.shown 恰為並排組合、至少 2 個）：#review 的 data-split＝實際可見的欄數；各欄同一列（上緣相同）、
//     由左到右依並排組合的順序（CSS order，不搬 DOM）、等寬（寬差 ≤ 1 px）、落在 #review 左右邊界內、各欄寬加間距
//     等於 #review 的寬（±2 px，確認真的切成等寬欄而不是縮在左邊）；只有焦點欄（目前分頁）的面板帶 data-split-focus，
//     其外框是 --accent、其他欄不是；各欄內容沒有超出欄寬（spec「某欄內容超出欄寬時，在該欄內捲動或折行」：面板本身
//     不橫向溢出，檢視器捲動容器以外的元素右緣不超出面板內緣）。
//   - 沒有並排顯示時：#review 沒有 data-split，也沒有任何元素帶 data-split-focus。
//   - 兩種情況都驗：頁面沒有橫向捲軸；#review 的寬（中欄寬度）與本段第一次量到的值相同（≤ 0.5 px；同一段內視窗大小
//     不變，第一次量在並排之前）。
async function checkLayout(ctx, exp, label) {
  const L = await ctx.cdp.run(() => window.__sc.layoutState());
  const splitShown = exp.cols.length >= 2 && exp.shown.length === exp.cols.length && exp.shown.every((n) => exp.cols.includes(n));
  const fmt = (ps) => JSON.stringify(ps.map((p) => `${p.name}@${Math.round(p.left)}+${Math.round(p.width * 10) / 10}`));
  check(!L.page.hScroll, `${label}：頁面沒有橫向捲軸（scrollWidth ${L.page.scrollWidth}、clientWidth ${L.page.clientWidth}）`);
  if (ctx.baseReviewWidth === undefined) ctx.baseReviewWidth = L.review.width;
  check(
    Math.abs(L.review.width - ctx.baseReviewWidth) <= 0.5,
    `${label}：中欄寬度（#review）與本段第一次量到的相同（前 ${ctx.baseReviewWidth}、後 ${L.review.width}）`
  );
  if (!splitShown) {
    check(L.split === null, `${label}：沒有並排顯示時 #review 沒有 data-split（實際 ${JSON.stringify(L.split)}）`);
    check(L.focusMarked.length === 0, `${label}：沒有並排顯示時沒有元素帶 data-split-focus（實際 ${JSON.stringify(L.focusMarked)}）`);
    return L;
  }
  const n = exp.cols.length;
  check(L.split === String(n), `${label}：#review 的 data-split＝${n}（實際 ${JSON.stringify(L.split)}）`);
  const ps = L.panels.filter((p) => exp.cols.includes(p.name));
  need(ps.length === n, `${label}：量得到 ${n} 個並排欄的版面框（實際 ${fmt(L.panels)}）`);
  const byLeft = [...ps].sort((a, b) => a.left - b.left);
  check(
    JSON.stringify(byLeft.map((p) => p.name)) === JSON.stringify(exp.cols),
    `${label}：由左到右依序為 ${JSON.stringify(exp.cols)}（實際 ${fmt(byLeft)}）`
  );
  const tops = ps.map((p) => p.top);
  check(Math.max(...tops) - Math.min(...tops) <= 1, `${label}：各欄在同一列（上緣 ${JSON.stringify(tops.map((t) => Math.round(t)))}）`);
  for (let i = 1; i < byLeft.length; i++) {
    check(byLeft[i].left >= byLeft[i - 1].right - 0.5, `${label}：${byLeft[i - 1].name} 與 ${byLeft[i].name} 左右相鄰不重疊（${fmt([byLeft[i - 1], byLeft[i]])}）`);
  }
  const widths = ps.map((p) => p.width);
  check(Math.max(...widths) - Math.min(...widths) <= 1, `${label}：${n} 欄等寬（寬差 ≤ 1 px；${fmt(byLeft)}）`);
  check(
    byLeft[0].left >= L.review.left - 0.5 && byLeft[n - 1].right <= L.review.right + 0.5,
    `${label}：各欄都在 #review 左右邊界內（#review ${Math.round(L.review.left)}～${Math.round(L.review.right)}；${fmt(byLeft)}）`
  );
  const filled = widths.reduce((a, b) => a + b, 0) + (n - 1) * L.gap;
  check(Math.abs(filled - L.review.width) <= 2, `${label}：各欄寬加間距等於 #review 的寬（${Math.round(filled * 10) / 10} vs ${L.review.width}，間距 ${L.gap}）`);
  check(
    JSON.stringify(L.focusMarked) === JSON.stringify([exp.selected]),
    `${label}：只有焦點欄 ${exp.selected} 的面板帶 data-split-focus（實際 ${JSON.stringify(L.focusMarked)}）`
  );
  const spilled = ps.filter((p) => p.overflow > 1 || p.spill.length > 0).map((p) => `${p.name}：溢出 ${p.overflow} px、${JSON.stringify(p.spill)}`);
  check(spilled.length === 0, `${label}：各欄內容沒有超出欄寬（面板本身沒有橫向溢出，工具列等不在檢視器捲動容器裡的元素右緣不超出面板；不符：${JSON.stringify(spilled)}）`);
  const wrongBorder = ps.filter((p) => (p.name === exp.selected) !== (p.border === L.accent)).map((p) => `${p.name}=${p.border}`);
  check(wrongBorder.length === 0, `${label}：焦點欄外框為 --accent（${L.accent}）、其他欄不是（不符：${JSON.stringify(wrongBorder)}）`);
  return L;
}

// #review 直屬子節點的順序（不得搬動面板的 DOM）。
const reviewChildOrder = (ctx) => ctx.cdp.run(() => window.__sc.reviewChildOrder());

// 某個檔案分頁的檢視器容器（.file-viewer-host，內容的捲動容器）：捲到 frac（0～1）的位置，等捲動事件讓 files.js
// 記下（ft.scroll），回傳實際的 scrollTop。要求內容確實比容器高（不然「捲到中段」不成立）。
async function scrollHostTo(ctx, rel, frac) {
  const r = await ctx.cdp.run((p, f) => {
    const host = window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('.file-viewer-host');
    const max = host.scrollHeight - host.clientHeight;
    if (max < 40) return { ok: false, max };
    host.scrollTop = Math.round(max * f);
    return { ok: true, max, top: host.scrollTop };
  }, rel, frac);
  need(r && r.ok, `${rel} 的內容比容器高、可以捲動（可捲範圍 ${r ? r.max : '?'} px）`);
  await sleep(300);
  const top = await hostScrollTop(ctx, rel);
  need(top > 0, `${rel} 已往下捲到中段（scrollTop ${top}，可捲範圍 ${r.max}）`);
  return top;
}

const hostScrollTop = (ctx, rel) => ctx.cdp.run((p) => window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('.file-viewer-host').scrollTop, rel);

// 在檢視器容器的第一個子節點上貼記號：之後記號還在＝內容節點沒有被重畫換掉。
const markHost = (ctx, rel, token) =>
  ctx.cdp.run((p, tk) => {
    const c = window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('.file-viewer-host').firstElementChild;
    if (!c) return false;
    c.__scMark = tk;
    return true;
  }, rel, token);
const hostMarked = (ctx, rel, token) =>
  ctx.cdp.run((p, tk) => {
    const c = window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('.file-viewer-host').firstElementChild;
    return !!c && c.__scMark === tk;
  }, rel, token);

// 某個檔案在 t0 之後的內容讀取（render／raw；不含中繼資料查詢）。
const contentReqsSince = (ctx, rel, t0) => ctx.net.reqs.filter((r) => r.filePath === rel && r.at > t0 && (r.kind === 'render' || r.kind === 'raw'));

// 斷言：rel 這一欄在 t0 之後沒有重新讀取內容、內容節點沒被換掉、捲動位置與 top 相同（容差 1 px）。
async function checkColumnUntouched(ctx, rel, t0, token, top, label) {
  const reads = contentReqsSince(ctx, rel, t0);
  check(reads.length === 0, `${label}：${rel} 沒有重新讀取內容（之後對它的 render／raw 請求 ${JSON.stringify(reads.map((r) => `${r.kind}@+${r.at - t0}ms`))}）`);
  check(await hostMarked(ctx, rel, token), `${label}：${rel} 的內容節點沒有被換掉（記號仍在）`);
  if (top !== null) {
    const now = await hostScrollTop(ctx, rel);
    check(Math.abs(now - top) <= 1, `${label}：${rel} 的捲動位置不變（前 ${top}、後 ${now}）`);
  }
}

// 開三欄 [a, b, c]：先選 a、按 b 的並排鈕、再按 c 的並排鈕，最後焦點欄＝focus（點它的分頁）。各檔需已打開。
async function buildThree(ctx, a, b, c, focus) {
  await clickTab(ctx, a);
  await pressSplit(ctx, b);
  await expectSplit(ctx, { cols: [a, b], selected: b, shown: [a, b] }, `前置：${a} 為目前分頁時按 ${b} 的並排鈕`);
  await pressSplit(ctx, c);
  await expectSplit(ctx, { cols: [a, b, c], selected: c, shown: [a, b, c] }, `前置：再按 ${c} 的並排鈕（未滿 3 個時加到最右欄）`);
  if (focus !== c) {
    await clickTab(ctx, focus);
    await expectSplit(ctx, { cols: [a, b, c], selected: focus, shown: [a, b, c] }, `前置：點選並排組合中的 ${focus}（只換焦點欄）`);
  }
}

// spec「加入並排」：已打開 README.md 與 docs/a.md，目前為 README.md；按 docs/a.md 的並排鈕 → 兩欄並排，依序
// README.md、docs/a.md，docs/a.md 為焦點欄與目前分頁，兩者的並排鈕 aria-pressed="true"。另驗：README.md 本來就可見，
// 加入並排時沒有重新讀取。
async function segSplitAdd() {
  const R = 'README.md';
  const A = 'docs/a.md';
  await withCockpit('split-add', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, '加入前');
    need(await markHost(ctx, R, 'add-R'), `在 ${R} 的內容節點貼記號`);
    const t0 = Date.now();
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `按 ${A} 的並排鈕後`);
    await checkColumnUntouched(ctx, R, t0, 'add-R', null, '加入並排');
    noExceptions(ctx);
  });
}

// spec「沒有另一個檔案分頁時不並排」與「加入」第二點（目前分頁就是 X 自己、是 Live Output，或是 git-review 定義的
// 分頁時，X 的並排鈕為停用狀態，以 title 說明需要先選另一個檔案分頁，按了不動作）：
//   1. 只打開 README.md 且它是目前分頁：並排鈕停用（expectDisabled），按了仍單欄。
//   2. 目前分頁是 Live Output：README.md 的並排鈕停用，按了不動作。
//   3. 目前分頁是 Git Graph（file-split-view task 3.3；審查第 3 項）：README.md 的並排鈕停用，按了不動作。
// 3.1 只驗「按了不動作」；停用呈現（aria-disabled、title、不用 disabled 屬性）與第 3 步是 task 3.3 加的。
async function segSplitNoPartner() {
  const R = 'README.md';
  await withCockpit('split-no-partner', {}, async (ctx) => {
    const zh = await dictTexts(ctx, 'zh');
    await openTree(ctx);
    await openReady(ctx, R);
    await expectDisabled(ctx, R, zh, `只有 ${R} 且它是目前分頁`);
    await pressSplit(ctx, R);
    await sleep(500);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, `只有 ${R} 且它是目前分頁時按它的並排鈕`);
    await clickTab(ctx, 'LIVE');
    await expectDisabled(ctx, R, zh, '目前分頁是 Live Output');
    await pressSplit(ctx, R);
    await sleep(500);
    await expectSplit(ctx, { cols: [], selected: 'LIVE', shown: ['LIVE'] }, `目前分頁是 Live Output 時按 ${R} 的並排鈕`);
    await openGitGraph(ctx);
    await expectSplit(ctx, { cols: [], selected: 'GRAPH', shown: ['GRAPH'] }, '打開 Git Graph 分頁後');
    await expectDisabled(ctx, R, zh, '目前分頁是 Git Graph');
    await pressSplit(ctx, R);
    await sleep(500);
    await expectSplit(ctx, { cols: [], selected: 'GRAPH', shown: ['GRAPH'] }, `目前分頁是 Git Graph 時按 ${R} 的並排鈕`);
    await expectDisabled(ctx, R, zh, `目前分頁是 Git Graph、按過 ${R} 的並排鈕之後`);
    noExceptions(ctx);
  });
}

// spec「替換焦點欄不影響其他欄」（a.md→long.md，要能捲到中段；docs/a.md 為第 2 欄焦點欄；docs/b.md→README.md）：
// long.md（第 1 欄，捲到中段）與 docs/a.md（第 2 欄、焦點欄）並排，另已打開 README.md；點選 README.md 分頁 →
// 第 2 欄改為 README.md 並為焦點欄；docs/a.md 分頁仍在分頁列、並排鈕 aria-pressed="false"；long.md 欄沒有重新讀取
// 內容（之後沒有對它的 render／raw 請求、內容節點沒換）、捲動位置不變。
async function segSplitReplaceKeepsOthers() {
  const L = 'long.md';
  const A = 'docs/a.md';
  const R = 'README.md';
  await withCockpit('split-replace', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, L);
    await openReady(ctx, A);
    await openReady(ctx, R);
    await clickTab(ctx, L);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [L, A], selected: A, shown: [L, A] }, `前置：${L} 與 ${A} 並排`);
    const top = await scrollHostTo(ctx, L, 0.4);
    need(await markHost(ctx, L, 'replace-L'), `在 ${L} 的內容節點貼記號`);
    const t0 = Date.now();
    await clickTab(ctx, R);
    const s = await expectSplit(ctx, { cols: [L, R], selected: R, shown: [L, R] }, `點選 ${R} 分頁後`);
    check(s.names.includes(A), `被取代的 ${A} 分頁仍在分頁列（實際 ${JSON.stringify(s.names)}）`);
    await sleep(500);
    await checkColumnUntouched(ctx, L, t0, 'replace-L', top, '替換焦點欄');
    noExceptions(ctx);
  });
}

// spec「從檔案樹開檔替換焦點欄」＋「替換」適用的另一個入口「點 md 相對連結開檔」（控制端要求三個入口都走通）：
//   1. docs/a.md（第 1 欄）與 README.md（第 2 欄、焦點欄）並排；在 README.md 欄點 [設計](docs/design.md#決策) →
//      新增 docs/design.md 分頁，取代第 2 欄並為焦點欄；README.md 分頁仍在；docs/a.md 欄沒有重新讀取。
//   2. 點選 docs/a.md 分頁（第 1 欄成為焦點欄），在檔案樹點尚未打開的 note.txt → 新增 note.txt 分頁，取代第 1 欄並為
//      焦點欄；docs/design.md 仍在第 2 欄、沒有重新讀取；docs/a.md 分頁仍在。
async function segSplitReplaceFromTreeAndLink() {
  const A = 'docs/a.md';
  const R = 'README.md';
  const D = 'docs/design.md';
  const N = 'note.txt';
  await withCockpit('split-tree', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, A);
    await openReady(ctx, R);
    await clickTab(ctx, A);
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [A, R], selected: R, shown: [A, R] }, `前置：${A} 與 ${R} 並排，${R} 為焦點欄`);
    need(await markHost(ctx, A, 'tree-A'), `在 ${A} 的內容節點貼記號`);
    const t0 = Date.now();
    const linkSel = `a[data-md-path="${D}"]`;
    need(
      await ctx.cdp.clickEl((p, sel) => window.__sc.panelOf(window.__sc.fileTab(p)).querySelector(sel), [R, linkSel], `${R} 欄的 md 相對連結 ${linkSel}`),
      `在 ${R} 欄點 md 相對連結（${D}#決策）`
    );
    const s1 = await expectSplit(ctx, { cols: [A, D], selected: D, shown: [A, D] }, `點 md 相對連結後`);
    check(s1.names.includes(R), `被取代的 ${R} 分頁仍在分頁列（實際 ${JSON.stringify(s1.names)}）`);
    await waitFileReady(ctx, D);
    await checkColumnUntouched(ctx, A, t0, 'tree-A', null, '點 md 相對連結替換焦點欄');

    await clickTab(ctx, A);
    await expectSplit(ctx, { cols: [A, D], selected: A, shown: [A, D] }, `點選並排組合中的 ${A}（只換焦點欄）`);
    need(await markHost(ctx, D, 'tree-D'), `在 ${D} 的內容節點貼記號`);
    const t1 = Date.now();
    await openFile(ctx, N);
    const s2 = await expectSplit(ctx, { cols: [N, D], selected: N, shown: [N, D] }, `在檔案樹點 ${N} 後`);
    check(s2.names.includes(A), `被取代的 ${A} 分頁仍在分頁列（實際 ${JSON.stringify(s2.names)}）`);
    check(s2.names.indexOf(N) === s2.names.length - 1, `${N} 是新增在分頁列最後面的分頁（實際 ${JSON.stringify(s2.names)}）`);
    await waitFileReady(ctx, N);
    await checkColumnUntouched(ctx, D, t1, 'tree-D', null, '從檔案樹開檔替換焦點欄');
    noExceptions(ctx);
  });
}

// spec「三欄已滿時替換焦點欄」（a→README.md、b→docs/a.md、c→long.md、d→note.txt）：三欄並排、焦點欄為 docs/a.md，
// 另已打開 note.txt；按 note.txt 的並排鈕 → 仍三欄，依序 README.md、note.txt、long.md，note.txt 為焦點欄；docs/a.md
// 分頁仍在。
async function segSplitFullReplace() {
  const [a, b, c, d] = ['README.md', 'docs/a.md', 'long.md', 'note.txt'];
  await withCockpit('split-full', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of [a, b, c, d]) await openReady(ctx, f);
    await buildThree(ctx, a, b, c, b);
    await pressSplit(ctx, d);
    const s = await expectSplit(ctx, { cols: [a, d, c], selected: d, shown: [a, d, c] }, `按 ${d} 的並排鈕後`);
    check(s.names.includes(b), `被取代的 ${b} 分頁仍在分頁列（實際 ${JSON.stringify(s.names)}）`);
    noExceptions(ctx);
  });
}

// spec「移出焦點欄」：README.md、docs/a.md、long.md 並排，焦點欄為 docs/a.md；按 docs/a.md 的並排鈕 → README.md、
// long.md 兩欄，long.md（右側欄）為焦點欄；docs/a.md 分頁仍在、aria-pressed="false"、不可見。
// 另驗沒有右側欄的情況（task 5.2 修正輪，延後 Minor 12b）：再按 docs/a.md 的並排鈕 → 加到最右欄、成為焦點欄（三欄
// README.md、long.md、docs/a.md）；再按一次移出 → 焦點欄改為左側的 long.md（不是第一欄 README.md）。
async function segSplitRemoveFocus() {
  const [a, b, c] = ['README.md', 'docs/a.md', 'long.md'];
  await withCockpit('split-remove', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of [a, b, c]) await openReady(ctx, f);
    await buildThree(ctx, a, b, c, b);
    await pressSplit(ctx, b);
    const s = await expectSplit(ctx, { cols: [a, c], selected: c, shown: [a, c] }, `按 ${b} 的並排鈕後`);
    check(s.names.includes(b), `${b} 分頁仍在分頁列（實際 ${JSON.stringify(s.names)}）`);
    await pressSplit(ctx, b);
    await expectSplit(ctx, { cols: [a, c, b], selected: b, shown: [a, c, b] }, `前置：再按 ${b} 的並排鈕（加到最右欄、成為焦點欄）`);
    await pressSplit(ctx, b);
    await expectSplit(ctx, { cols: [a, c], selected: c, shown: [a, c] }, `移出最右側的焦點欄 ${b} 後（沒有右側欄，焦點欄改為左側的 ${c}）`);
    noExceptions(ctx);
  });
}

// spec「關閉後只剩一個時解除並排」：README.md 與 docs/a.md 並排、焦點欄為 docs/a.md，另有未並排的 long.md 在
// docs/a.md 右側（分頁列順序 README.md、docs/a.md、long.md）；關閉 docs/a.md → 解除並排，README.md 單欄並為目前
// 分頁。若誤用「關閉目前分頁改為顯示右側分頁」的規則，會變成 long.md，所以這個佈置分得出兩條規則。
// 另驗並排中關閉非焦點欄（task 5.2 修正輪，延後 Minor 17b）：再打開 note.txt，README.md、long.md、note.txt 三欄並排、
// 焦點欄 README.md；用滑鼠按 long.md 的關閉鈕 → README.md、note.txt 兩欄，目前分頁仍是 README.md；鍵盤焦點移到被關分頁
// 右側相鄰的 note.txt，不是目前分頁 README.md（closeTab() 的「目前分頁不變」分支）。
async function segSplitCloseDissolves() {
  const [a, b, c, d] = ['README.md', 'docs/a.md', 'long.md', 'note.txt'];
  await withCockpit('split-close', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of [a, b, c]) await openReady(ctx, f);
    await clickTab(ctx, a);
    await pressSplit(ctx, b);
    await expectSplit(ctx, { cols: [a, b], selected: b, shown: [a, b] }, `前置：${a} 與 ${b} 並排`);
    await pressClose(ctx, b);
    const s = await expectSplit(ctx, { cols: [], selected: a, shown: [a] }, `關閉 ${b} 後`);
    check(!s.names.includes(b), `${b} 分頁已移除（實際 ${JSON.stringify(s.names)}）`);

    await openReady(ctx, d);
    await buildThree(ctx, a, c, d, a);
    await pressClose(ctx, c);
    const s2 = await expectSplit(ctx, { cols: [a, d], selected: a, shown: [a, d] }, `並排中關閉非焦點欄 ${c} 後（目前分頁不變）`);
    check(!s2.names.includes(c), `${c} 分頁已移除（實際 ${JSON.stringify(s2.names)}）`);
    const f = await ctx.cdp.poll((n) => window.__sc.focusName() === n, [d], UI_TIMEOUT_MS);
    check(!!f, `並排中關閉非焦點欄 ${c} 後鍵盤焦點移到右側相鄰的 ${d}，不是目前分頁 ${a}（實際 ${JSON.stringify(await ctx.cdp.run(() => window.__sc.focusName()))}）`);
    noExceptions(ctx);
  });
}

// spec「不在並排中時關閉並排組合的成員」：README.md、docs/a.md、long.md 並排、焦點欄為 docs/a.md，之後選定 Live Output；
// 關閉 docs/a.md → 目前分頁仍為 Live Output、只顯示 Live Output；再點選 README.md → README.md、long.md 兩欄並排，
// README.md 為焦點欄。
async function segSplitCloseWhileAway() {
  const [a, b, c] = ['README.md', 'docs/a.md', 'long.md'];
  await withCockpit('split-close-away', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of [a, b, c]) await openReady(ctx, f);
    await buildThree(ctx, a, b, c, b);
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [a, b, c], selected: 'LIVE', shown: ['LIVE'] }, '選定 Live Output 後（並排組合保留）');
    await pressClose(ctx, b);
    const s = await expectSplit(ctx, { cols: [a, c], selected: 'LIVE', shown: ['LIVE'] }, `關閉 ${b} 後`);
    check(!s.names.includes(b), `${b} 分頁已移除（實際 ${JSON.stringify(s.names)}）`);
    await clickTab(ctx, a);
    await expectSplit(ctx, { cols: [a, c], selected: a, shown: [a, c] }, `再點選 ${a} 後`);
    noExceptions(ctx);
  });
}

// spec「不在並排中時加入已滿的並排組合」：README.md、docs/a.md、long.md 並排、焦點欄為 docs/a.md，之後選定 Live Output，
// 另已打開 note.txt；按 note.txt 的並排鈕 → 依序 README.md、note.txt、long.md 三欄並排顯示，note.txt 為焦點欄與目前分頁。
async function segSplitAddFullWhileAway() {
  const [a, b, c, d] = ['README.md', 'docs/a.md', 'long.md', 'note.txt'];
  await withCockpit('split-add-away', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of [a, b, c, d]) await openReady(ctx, f);
    await buildThree(ctx, a, b, c, b);
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [a, b, c], selected: 'LIVE', shown: ['LIVE'] }, '選定 Live Output 後（並排組合保留）');
    await pressSplit(ctx, d);
    const s = await expectSplit(ctx, { cols: [a, d, c], selected: d, shown: [a, d, c] }, `按 ${d} 的並排鈕後`);
    check(s.names.includes(b), `被取代的 ${b} 分頁仍在分頁列（實際 ${JSON.stringify(s.names)}）`);
    noExceptions(ctx);
  });
}

// spec「切到 Live Output 後整組恢復」（a.md→long.md、b.md→docs/design.md，兩者都夠長、能捲到中段）：兩欄並排、都往下
// 捲到中段；點選 Live Output → 只顯示 Live Output（並排組合保留）；再點選 long.md → 兩欄恢復並排，long.md 為焦點欄，
// 兩欄捲動位置與離開前相同，而且都沒有重新讀取內容（之後沒有 render／raw 請求、內容節點沒換）。
async function segSplitRestoreAfterLive() {
  const L = 'long.md';
  const D = 'docs/design.md';
  await withCockpit('split-live', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, L);
    await openReady(ctx, D);
    await clickTab(ctx, L);
    await pressSplit(ctx, D);
    await expectSplit(ctx, { cols: [L, D], selected: D, shown: [L, D] }, `前置：${L} 與 ${D} 並排`);
    const topL = await scrollHostTo(ctx, L, 0.4);
    const topD = await scrollHostTo(ctx, D, 0.5);
    need(await markHost(ctx, L, 'live-L'), `在 ${L} 的內容節點貼記號`);
    need(await markHost(ctx, D, 'live-D'), `在 ${D} 的內容節點貼記號`);
    const t0 = Date.now();
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [L, D], selected: 'LIVE', shown: ['LIVE'] }, '點選 Live Output 期間');
    await clickTab(ctx, L);
    await expectSplit(ctx, { cols: [L, D], selected: L, shown: [L, D] }, `再點選 ${L} 後`);
    await sleep(500);
    await checkColumnUntouched(ctx, L, t0, 'live-L', topL, '整組恢復');
    await checkColumnUntouched(ctx, D, t0, 'live-D', topD, '整組恢復');
    noExceptions(ctx);
  });
}

// spec「非檔案分頁不能並排」：README.md 與 docs/a.md 並排；從左欄「變更」打開 Git Graph 分頁 → Git Graph 成為目前分頁並
// 單獨顯示，並排組合不變（Git Graph 沒有取代焦點欄）；Git Graph 與 Live Output 分頁上都沒有並排鈕；再點選 README.md →
// 兩欄恢復並排、README.md 為焦點欄。
// file-split-view task 3.3 加：
//   - 「沒有並排鈕」改用 splitButtonsIn()：帶 aria-pressed 的 <button> 與 .review-tab-split 都算（審查第 2 項）。另打開一個
//     diff 分頁（左欄「變更」的 history/unstaged-change.txt），它也不得有並排鈕。
//   - 並排中按住 Ctrl 點選 Git Graph → Git Graph 成為目前分頁並單獨顯示，並排組合不變。
//   - spec 的 GIVEN 原樣：移出 docs/a.md 使並排組合解除、目前為 README.md，按住 Ctrl 點選 Git Graph → Git Graph 成為目前
//     分頁並單獨顯示，沒有形成並排。
async function segSplitNonFileTabs() {
  const R = 'README.md';
  const A = 'docs/a.md';
  const DIFF = 'DIFF:history/unstaged-change.txt';
  await withCockpit('split-nonfile', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await clickTab(ctx, R);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `前置：${R} 與 ${A} 並排`);
    await openGitGraph(ctx);
    const s = await expectSplit(ctx, { cols: [R, A], selected: 'GRAPH', shown: ['GRAPH'] }, '打開 Git Graph 分頁後');
    check(s.buttons.GRAPH === false, `Git Graph 分頁上沒有並排鈕（含 .review-tab-split；實際 ${JSON.stringify(s.buttons)}）`);
    check(s.buttons.LIVE === false, `Live Output 分頁上沒有並排鈕（含 .review-tab-split；實際 ${JSON.stringify(s.buttons)}）`);
    check(s.buttons[R] === true && s.buttons[A] === true, `檔案分頁上都有並排鈕（實際 ${JSON.stringify(s.buttons)}）`);
    // diff 分頁（git 類的另一種）
    const rowSel = '#changes-panel .changes-row[title="history/unstaged-change.txt"]';
    need(!!(await ctx.cdp.poll((sel) => !!document.querySelector(sel), [rowSel], UI_TIMEOUT_MS)), '「變更」面板列出 history/unstaged-change.txt');
    need(await ctx.cdp.clickEl((sel) => document.querySelector(sel), [rowSel], '「變更」列 history/unstaged-change.txt'), '點「變更」列 history/unstaged-change.txt');
    const s2 = await expectSplit(ctx, { cols: [R, A], selected: DIFF, shown: [DIFF] }, '打開 diff 分頁後');
    check(s2.buttons[DIFF] === false, `diff 分頁上沒有並排鈕（含 .review-tab-split；實際 ${JSON.stringify(s2.buttons)}）`);
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [R, A], selected: R, shown: [R, A] }, `再點選 ${R} 後`);
    // 並排中 Ctrl＋點選 Git Graph：等同一般選定，Git Graph 單獨顯示、並排組合不變。
    await clickTab(ctx, 'GRAPH', { modifiers: MOD.CTRL });
    await expectSplit(ctx, { cols: [R, A], selected: 'GRAPH', shown: ['GRAPH'] }, '並排中按住 Ctrl 點選 Git Graph 後');
    // spec 的 GIVEN：沒有並排組合、目前為 README.md。
    await clickTab(ctx, R);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, `移出 ${A}，並排組合解除、目前為 ${R}`);
    await clickTab(ctx, 'GRAPH', { modifiers: MOD.CTRL });
    await expectSplit(ctx, { cols: [], selected: 'GRAPH', shown: ['GRAPH'] }, `目前為 ${R}、沒有並排組合時按住 Ctrl 點選 Git Graph 後`);
    noExceptions(ctx);
  });
}

// spec「並排中的非焦點欄也更新」：docs/a.md（第 1 欄）與 README.md（第 2 欄、焦點欄）並排；在 docs/a.md 末端加一段文字
// （只寫 ui_preview 的暫存副本）→ 3 秒內第 1 欄出現新段落，焦點欄仍為 README.md。
async function segSplitNonFocusUpdates() {
  const A = 'docs/a.md';
  const R = 'README.md';
  await withCockpit('split-update', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, A);
    await openReady(ctx, R);
    await clickTab(ctx, A);
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [A, R], selected: R, shown: [A, R] }, `前置：${A} 與 ${R} 並排，${R} 為焦點欄`);
    const marker = `SPLIT-UPDATE-${Date.now()}`;
    const t0 = Date.now();
    fs.appendFileSync(path.join(ctx.preview.reviewRepo, ...A.split('/')), `\n\n${marker} 新段落。\n`);
    const shown = await ctx.cdp.poll((p, m) => window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('.file-viewer-host').textContent.includes(m), [A, marker], 3000, 50);
    check(!!shown, `3 秒內第 1 欄（${A}）出現新段落（${shown ? `${Date.now() - t0} ms` : '逾時'}）`);
    await expectSplit(ctx, { cols: [A, R], selected: R, shown: [A, R] }, '新段落出現後');
    noExceptions(ctx);
  });
}

// 並排中各欄的輪詢互不影響（file-split-view task 2.3 留下的實證缺口；spec「自動更新」：每個可見的檔案分頁各自定時查詢；
// design D2 的 openFile() error 重試條件＝「已經可見」；design D3）。README.md、docs/a.md、long.md 三欄並排、焦點欄為
// long.md，另已打開未並排的 note.txt。
//   1. 觀察 6 秒：三欄（含兩個非焦點欄）各至少 2 次中繼資料查詢，note.txt 0 次。
//   2. 攔住 docs/a.md（可見、非焦點欄）的查詢，等一筆卡住後關閉它 → README.md、long.md 兩欄，焦點欄仍為 long.md；
//      卡住的那筆 1 秒內被頁面中止、放行時已不存在；之後觀察 6 秒，README.md、long.md 照常查詢，docs/a.md 沒有新的請求。
//   3. 刪掉 README.md（暫存副本）讓它變成 error；等它的一次查詢回來後立刻在檔案樹點 README.md（可見、非焦點、error 的欄）
//      → README.md 成為焦點欄，而且立即重試：點擊後、下一次排定的查詢（回應後 2 秒）之前，就有一筆 README.md 的查詢。
//      之後觀察 6 秒，long.md 照常查詢（重試只重開 README.md 自己的鏈）。
async function segSplitPollingIndependent() {
  const [R, A, L, N] = ['README.md', 'docs/a.md', 'long.md', 'note.txt'];
  const windowMs = 3 * POLL_INTERVAL_MS;
  const minHits = 2;
  await withCockpit('split-poll', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of [R, A, L, N]) await openReady(ctx, f);
    await buildThree(ctx, R, A, L, L);

    const seen1 = await observeMeta(ctx, windowMs);
    log(`三欄並排：${seen1.windowMs} ms 內的中繼資料查詢 ${JSON.stringify(seen1.byPath)}`);
    for (const f of [R, A, L]) check((seen1.byPath[f] || 0) >= minHits, `三欄並排時 ${f}${f === L ? '（焦點欄）' : '（非焦點欄）'} 在 ${windowMs} ms 內被查詢至少 ${minHits} 次（實際 ${seen1.byPath[f] || 0}）`);
    check((seen1.byPath[N] || 0) === 0, `未並排、不可見的 ${N} 沒有被查詢（實際 ${seen1.byPath[N] || 0}）`);

    const hold = await installMetaHold(ctx, A);
    await waitFirstHeld(hold, A);
    const tClose = await pressClose(ctx, A);
    const heldIds = hold.held.map((h) => h.networkId);
    await expectSplit(ctx, { cols: [R, L], selected: L, shown: [R, L] }, `關閉可見的非焦點欄 ${A} 後`);
    const r = await waitCanceled(ctx, hold.held[0].networkId, 1000);
    check(!!r && r.canceled, `關閉後 1 秒內，${A} 卡住的查詢被頁面中止（實際 ${JSON.stringify(r ? { canceled: r.canceled, errorText: r.errorText, after: r.failedAt - tClose } : null)}）`);
    await hold.release(STALE_RESPONSE.status, STALE_RESPONSE.body);
    hold.held.forEach((h, i) => check(h.outcome === 'gone', `攔下的第 ${i + 1} 筆 ${A} 查詢放行時 Chrome 回報已不存在（實際 ${h.outcome}${h.error ? `：${h.error}` : ''}）`));
    const seen2 = await observeMeta(ctx, windowMs);
    log(`關閉 ${A} 後：${seen2.windowMs} ms 內的中繼資料查詢 ${JSON.stringify(seen2.byPath)}`);
    for (const f of [R, L]) check((seen2.byPath[f] || 0) >= minHits, `關閉 ${A} 後 ${f} 照常查詢（${windowMs} ms 內至少 ${minHits} 次，實際 ${seen2.byPath[f] || 0}）`);
    const lateA = fileReqsSince(ctx, A, tClose, heldIds);
    check(lateA.length === 0, `關閉後 ${A} 沒有新的請求（實際 ${JSON.stringify(lateA.map((x) => `${x.kind}@+${x.at - tClose}ms`))}）`);

    fs.unlinkSync(path.join(ctx.preview.reviewRepo, R));
    const errored = await ctx.cdp.poll((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const st = panel ? panel.querySelector('.file-status') : null;
      return !!st && !st.hidden && st.getAttribute('data-tone') === 'warn';
    }, [R], POLL_INTERVAL_MS * 2 + UI_TIMEOUT_MS);
    need(!!errored, `刪掉 ${R} 後它的欄進入 error（狀態列以 warn 顯示原因）`);
    // 等 README.md 的下一次查詢回來：下一次排定的查詢在回應後 2 秒，點擊落在這段空檔的開頭。
    const tWait = Date.now();
    let tResp = null;
    while (Date.now() - tWait < POLL_INTERVAL_MS + UI_TIMEOUT_MS) {
      const hit = ctx.net.reqs.filter((x) => x.metaPath === R && x.respondedAt !== null && x.respondedAt > tWait);
      if (hit.length > 0) {
        tResp = hit[hit.length - 1].respondedAt;
        break;
      }
      await sleep(20);
    }
    need(tResp !== null, `等到 ${R} 的一次中繼資料查詢回來`);
    const tClick = Date.now();
    need(await ctx.cdp.clickEl((p) => window.__sc.row(p), [R], `檔案列 ${R}`), `在檔案樹點 ${R}（可見、非焦點、error 的欄）`);
    await expectSplit(ctx, { cols: [R, L], selected: R, shown: [R, L] }, `點 ${R} 的檔案列後（只換焦點欄）`);
    const retry = ctx.net.reqs.filter((x) => x.metaPath === R && x.at >= tClick && x.at < tResp + POLL_INTERVAL_MS - 500);
    check(
      retry.length >= 1,
      `點擊後立即重試：在下一次排定的查詢（回應後 ${POLL_INTERVAL_MS} ms）之前就有 ${R} 的查詢（點擊於回應後 ${tClick - tResp} ms；之後的 ${R} 查詢 ${JSON.stringify(
        ctx.net.reqs.filter((x) => x.metaPath === R && x.at >= tClick).map((x) => `+${x.at - tResp}ms`)
      )}，相對回應時間）`
    );
    const seen3 = await observeMeta(ctx, windowMs);
    log(`重試 ${R} 後：${seen3.windowMs} ms 內的中繼資料查詢 ${JSON.stringify(seen3.byPath)}`);
    check((seen3.byPath[L] || 0) >= minHits, `重試 ${R} 後 ${L} 照常查詢（${windowMs} ms 內至少 ${minHits} 次，實際 ${seen3.byPath[L] || 0}）`);
    check((seen3.byPath[R] || 0) >= minHits, `重試後 ${R} 依原節奏繼續查詢（實際 ${seen3.byPath[R] || 0}）`);
    noExceptions(ctx);
  });
}

// ---------------------------------------------------------------------------
// 並排版面（file-split-view task 3.2；design D4、Risks 第一條；spec「檔案並排」的「版面」）
//
// 幾何斷言由 checkLayout() 負責（expectSplit() 每次都呼叫，所以 3.1 的狀態段落也一併驗版面）。本節的段落另外驗：
// 1280 寬時的等寬、欄位順序靠 CSS order 而不是搬 DOM、三欄加長行與 PDF 不撐破頁面、html 的 iframe 不重新載入、
// 整頁重畫與切換 Project 不影響並排。視窗一律 1280×900（spec「三欄並排不撐破頁面」的 GIVEN；≥1200 寬且 ≥720 高，
// 是「固定一屏」版面）。
// ---------------------------------------------------------------------------

const W1280 = '1280,900';

// 把版面寬度固定為 1280×900 並確認。headless 的 --window-size=1280,900 實測 innerWidth 只有 1258，所以改用
// Emulation.setDeviceMetricsOverride（同 visual-check.js 的做法），再量 innerWidth 確認。
async function need1280(ctx) {
  const r = await ctx.cdp.send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  need(!r.error, `Emulation.setDeviceMetricsOverride 1280×900（${r.error ? JSON.stringify(r.error) : 'ok'}）`);
  await sleep(150);
  const w = await ctx.cdp.run(() => window.innerWidth);
  need(w === 1280, `視窗寬度為 1280（innerWidth 實際 ${w}）`);
}

// 在面板（tabpanel 元素本身）上貼記號：之後記號還在＝面板節點沒有被換掉。
const markPanels = (ctx, names, token) =>
  ctx.cdp.run((ns, tk) => ns.every((n) => {
    const p = window.__sc.panelOf(window.__sc.fileTab(n));
    if (!p) return false;
    p.__scPanelMark = tk;
    return true;
  }), names, token);
const unmarkedPanels = (ctx, names, token) =>
  ctx.cdp.run((ns, tk) => ns.filter((n) => {
    const p = window.__sc.panelOf(window.__sc.fileTab(n));
    return !p || !p.isConnected || p.__scPanelMark !== tk;
  }), names, token);

// 斷言 #review 直屬子節點的順序與 before 相同（並排不搬動面板的 DOM；design D4）。
async function checkChildOrder(ctx, before, label) {
  const now = await reviewChildOrder(ctx);
  check(JSON.stringify(now) === JSON.stringify(before), `${label}：#review 子節點順序沒有變（沒有搬動面板的 DOM；前 ${JSON.stringify(before)}、後 ${JSON.stringify(now)}）`);
}

// spec「加入並排」（等寬兩欄、左右順序）、「三欄已滿時替換焦點欄」（替換後的欄位順序）與「版面」；design D4（grid、
// CSS order、不搬 DOM、焦點欄外框）。視窗 1280×900，README.md、docs/a.md、long.md、note.txt 依序打開。
//   1. 只有 README.md 時量中欄寬度（checkLayout 的基準）。
//   2. README.md 時按 docs/a.md 的並排鈕 → 等寬兩欄，左 README.md、右 docs/a.md。
//   3. 再按 long.md 的並排鈕 → 等寬三欄。
//   4. 點 docs/a.md（焦點欄移到中間），按 note.txt 的並排鈕 → 依序 README.md、note.txt、long.md。note.txt 的面板在
//      DOM 裡排在 long.md 之後，畫面上卻要在中間：只有用 CSS order 才做得到（搬 DOM 會被 #review 子節點順序抓到）。
//   5. 選 Live Output → 單欄、沒有 data-split；再點 note.txt → 三欄恢復、焦點欄外框移到 note.txt。
//   每一步 expectSplit（含 checkLayout）＋#review 子節點順序不變。
async function segSplitLayoutEqual() {
  const [R, A, L, N] = ['README.md', 'docs/a.md', 'long.md', 'note.txt'];
  await withCockpit('split-layout', { windowSize: W1280 }, async (ctx) => {
    await need1280(ctx);
    await openTree(ctx);
    for (const f of [R, A, L, N]) await openReady(ctx, f);
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, '並排前（只有 README.md 可見）');
    const order0 = await reviewChildOrder(ctx);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `按 ${A} 的並排鈕後（兩欄）`);
    log(`兩欄：${JSON.stringify((await ctx.cdp.run(() => window.__sc.layoutState())).panels.map((p) => [p.name, p.left, p.width]))}`);
    await checkChildOrder(ctx, order0, '兩欄');
    await pressSplit(ctx, L);
    await expectSplit(ctx, { cols: [R, A, L], selected: L, shown: [R, A, L] }, `按 ${L} 的並排鈕後（三欄）`);
    log(`三欄：${JSON.stringify((await ctx.cdp.run(() => window.__sc.layoutState())).panels.map((p) => [p.name, p.left, p.width]))}`);
    await checkChildOrder(ctx, order0, '三欄');
    await clickTab(ctx, A);
    await expectSplit(ctx, { cols: [R, A, L], selected: A, shown: [R, A, L] }, `點 ${A}（焦點欄移到中間）`);
    await pressSplit(ctx, N);
    await expectSplit(ctx, { cols: [R, N, L], selected: N, shown: [R, N, L] }, `按 ${N} 的並排鈕後（取代中間的焦點欄）`);
    await checkChildOrder(ctx, order0, '替換中間欄');
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [R, N, L], selected: 'LIVE', shown: ['LIVE'] }, '選 Live Output（不在並排中）');
    await clickTab(ctx, N);
    await expectSplit(ctx, { cols: [R, N, L], selected: N, shown: [R, N, L] }, `再點 ${N}（三欄恢復）`);
    await checkChildOrder(ctx, order0, '離開再回到並排');
    noExceptions(ctx);
  });
}

// spec「三欄並排不撐破頁面」＋design Risks 第一條（PDF 符合寬度在欄寬改變後重排）。a.md→README.md；long.txt 寫進
// ui_preview 的暫存副本（第 2 行是 300 個字元的長行，fixture 沒有這種檔案，不改 cockpit/examples/fixtures/）；
// report.pdf 是 fixture 的 3 頁 PDF。推送間隔 1 秒，用來觸發 spec 的「重畫」。
//   1. 只有 report.pdf 可見時：PDF 為符合寬度（工具列「符合寬度」aria-pressed="true"、檢視器狀態 zoom＝"fit"、最寬一頁
//      ＝捲動區寬減左右內距 2×12 px），記下頁寬。
//   2. README.md、long.txt、report.pdf 三欄並排（焦點欄 report.pdf），等一次整頁重畫。
//   3. 斷言：checkLayout（無橫向捲軸、中欄寬度不變、等寬）；long.txt 的長行在該欄內捲動（.file-viewer-host 的
//      scrollWidth 大於 clientWidth、設 scrollLeft 真的捲得動、面板本身沒有溢出）；PDF 仍為符合寬度，最寬一頁＝新的
//      捲動區寬減 24 px，頁寬比單欄時小（欄寬真的變了），PDF 捲動區沒有橫向溢出。
const LONG_TXT = 'long.txt';
const LONG_LINE = '0123456789'.repeat(30); // 300 個字元
async function segSplitThreeNoOverflow() {
  const [R, T, P] = ['README.md', LONG_TXT, 'report.pdf'];
  await withCockpit('split-overflow', { windowSize: W1280, env: { COCKPIT_PREVIEW_PUSH_MS: '1000' } }, async (ctx) => {
    await need1280(ctx);
    fs.writeFileSync(path.join(ctx.preview.reviewRepo, T), `split-check：三欄並排不撐破頁面\n${LONG_LINE}\n第 3 行\n`);
    need(LONG_LINE.length === 300, `長行是 300 個字元（實際 ${LONG_LINE.length}）`);
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, T);
    await openFile(ctx, P);
    const pdf0 = await waitPdfFit(ctx, P, null, '單欄時');
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, '並排前（只有 README.md 可見，量中欄寬度）');
    await pressSplit(ctx, T);
    await pressSplit(ctx, P);
    const s = await expectSplit(ctx, { cols: [R, T, P], selected: P, shown: [R, T, P] }, `${R}、${T}、${P} 三欄並排`);
    need(!!s, '三欄並排成立');
    const v0 = await stateVersion(ctx);
    const v1 = await waitForRepaint(ctx, v0, 5000);
    need(v1 !== null, `三欄並排後發生整頁重畫（data-state-version ${v0} → ${v1}）`);
    await sleep(300);
    await expectSplit(ctx, { cols: [R, T, P], selected: P, shown: [R, T, P] }, '整頁重畫後');
    const t = await ctx.cdp.run((p, line) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const host = panel.querySelector('.file-viewer-host');
      const content = host.querySelector('.text-content');
      const before = host.scrollLeft;
      host.scrollLeft = 120;
      const moved = host.scrollLeft;
      host.scrollLeft = before;
      // 診斷：面板內（檢視器捲動容器之外）右緣超出面板內緣的元素。
      const pr = panel.getBoundingClientRect();
      const innerRight = pr.left + panel.clientLeft + panel.clientWidth;
      const overflowers = Array.from(panel.querySelectorAll('*'))
        .filter((el) => !host.contains(el) && el.getClientRects().length > 0 && el.getBoundingClientRect().right > innerRight + 0.5)
        .map((el) => `${el.tagName.toLowerCase()}.${String(el.className).replace(/ /g, '.')}@${Math.round(el.getBoundingClientRect().right - innerRight)}px`);
      return {
        overflowers,
        hasLine: !!content && content.textContent.includes(line),
        hostScrollWidth: host.scrollWidth,
        hostClientWidth: host.clientWidth,
        moved,
        panelScrollWidth: panel.scrollWidth,
        panelClientWidth: panel.clientWidth,
        panelWidth: panel.getBoundingClientRect().width,
      };
    }, T, LONG_LINE);
    need(t.hasLine, `${T} 的內容含那一行 300 個字元的長行`);
    check(t.hostScrollWidth > t.hostClientWidth, `${T} 的長行比欄寬長，在該欄的內容容器內溢出（scrollWidth ${t.hostScrollWidth} > clientWidth ${t.hostClientWidth}）`);
    check(t.moved > 0, `${T} 的內容容器可以橫向捲動（設 scrollLeft＝120，實際 ${t.moved}）`);
    check(t.panelScrollWidth <= t.panelClientWidth + 1, `${T} 的面板本身沒有溢出（面板 scrollWidth ${t.panelScrollWidth}、clientWidth ${t.panelClientWidth}；超出面板內緣的元素 ${JSON.stringify(t.overflowers)}）`);
    const pdf1 = await waitPdfFit(ctx, P, pdf0.pageWidth, '三欄並排後');
    check(pdf1.pageWidth < pdf0.pageWidth - 50, `${P} 的頁寬隨欄寬變小（單欄 ${pdf0.pageWidth}、三欄 ${pdf1.pageWidth}）`);
    check(pdf1.scrollWidth <= pdf1.clientWidth + 1, `${P} 的捲動區沒有橫向溢出（scrollWidth ${pdf1.scrollWidth}、clientWidth ${pdf1.clientWidth}）`);
    noExceptions(ctx);
  });
}

// 等 PDF 檢視器排好、處於「符合寬度」：檢視器狀態 zoom＝"fit"、工具列帶 aria-pressed 的「符合寬度」為 "true"、最寬一頁
// 的寬＝捲動區 clientWidth − 2×12（viewers.js 的 PDF_PAD_PX），容差 1 px。notWidth 不是 null 時，另等頁寬不再等於它
// （欄寬改變後 ResizeObserver 重排完成）。回傳量測值。
async function waitPdfFit(ctx, rel, notWidth, label) {
  const probe = (p, nw) => {
    const panel = window.__sc.panelOf(window.__sc.fileTab(p));
    const host = panel && panel.querySelector('.file-viewer-host');
    const view = host && host.querySelector('[data-viewer="pdf"]');
    const sc = view && view.querySelector('.pdf-pages');
    if (!sc || !view.querySelector('.pdf-page > canvas')) return false;
    const st = window.cockpitViewerHost.state(host);
    const fit = view.querySelector('.pdf-toolbar button[aria-pressed]');
    const pageWidth = Math.max(...Array.from(sc.querySelectorAll('.pdf-page')).map((b) => b.getBoundingClientRect().width));
    const r = {
      zoom: st ? st.zoom : null,
      pressed: fit ? fit.getAttribute('aria-pressed') : null,
      pageWidth,
      clientWidth: sc.clientWidth,
      scrollWidth: sc.scrollWidth,
    };
    const ok = r.zoom === 'fit' && r.pressed === 'true' && Math.abs(pageWidth - (sc.clientWidth - 24)) <= 1 && (nw === null || Math.abs(pageWidth - nw) > 1);
    return ok ? r : false;
  };
  const r = await ctx.cdp.poll(probe, [rel, notWidth], 10000);
  const last = r || (await ctx.cdp.run((p) => {
    const panel = window.__sc.panelOf(window.__sc.fileTab(p));
    const host = panel && panel.querySelector('.file-viewer-host');
    const sc = host && host.querySelector('.pdf-pages');
    const st = host ? window.cockpitViewerHost.state(host) : null;
    return { zoom: st ? st.zoom : null, clientWidth: sc ? sc.clientWidth : null, pages: sc ? Array.from(sc.querySelectorAll('.pdf-page')).map((b) => b.getBoundingClientRect().width) : null };
  }, rel));
  check(!!r, `${label}：${rel} 為符合寬度（zoom＝"fit"、「符合寬度」aria-pressed="true"、最寬一頁＝捲動區寬 − 24 px${notWidth !== null ? `、頁寬已不是 ${notWidth}` : ''}；${JSON.stringify(last)}）`);
  if (!r) throw new SegmentAbort();
  return r;
}

// design D4「不搬 DOM」的理由：搬動 <iframe> 會讓瀏覽器重新載入它。page.html（html 檢視器，sandbox iframe）與
// README.md：在 iframe 元素上貼記號並掛 load 計數，記下之後對 page.html 的 raw 請求（iframe 的導覽）。操作：
// 並排（page.html 在右欄）→ 選 Live Output → 回到並排 → 再按 long.md 的並排鈕成為三欄 → 點 README.md 換焦點欄 →
// 按 page.html 的並排鈕把它移出（剩 README.md、long.md 兩欄）→ 選 Live Output → 點 page.html 單欄顯示。每一步之後：iframe 仍是同一個
// 節點（記號在、仍連在文件上、是 page.html 面板裡唯一的 iframe）、load 事件 0 次、沒有 page.html 的 raw 請求。
async function segSplitIframeNotReloaded() {
  const [H, R, L] = ['page.html', 'README.md', 'long.md'];
  await withCockpit('split-iframe', { windowSize: W1280 }, async (ctx) => {
    await need1280(ctx);
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, L);
    await openFile(ctx, H);
    const ready = await ctx.cdp.poll((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const view = panel && panel.querySelector('[data-viewer="html"]');
      const f = view && !view.hidden ? view.querySelector('iframe') : null;
      return !!f && panel.querySelectorAll('iframe').length === 1;
    }, [H], 10000);
    need(!!ready, `${H} 的 html 檢視器已顯示 iframe（面板內恰一個 iframe）`);
    await sleep(500);
    need(
      await ctx.cdp.run((p) => {
        const f = window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('iframe');
        f.__scFrame = 'iframe-1';
        window.__scFrameLoads = 0;
        f.addEventListener('load', () => {
          window.__scFrameLoads += 1;
        });
        return true;
      }, H),
      `在 ${H} 的 iframe 上貼記號、掛 load 計數`
    );
    const t0 = Date.now();
    const frameCheck = async (label) => {
      const f = await ctx.cdp.run((p) => {
        const panel = window.__sc.panelOf(window.__sc.fileTab(p));
        const frames = panel ? panel.querySelectorAll('iframe') : [];
        const fr = frames[0] || null;
        return { count: frames.length, marked: !!fr && fr.__scFrame === 'iframe-1', connected: !!fr && fr.isConnected, loads: window.__scFrameLoads };
      }, H);
      check(f.count === 1 && f.marked && f.connected, `${label}：${H} 的 iframe 仍是同一個節點（${JSON.stringify(f)}）`);
      check(f.loads === 0, `${label}：${H} 的 iframe 沒有重新載入（load 事件 ${f.loads} 次）`);
      const raws = ctx.net.reqs.filter((r) => r.filePath === H && r.at > t0 && r.kind === 'raw');
      check(raws.length === 0, `${label}：之後沒有 ${H} 的 raw 請求（iframe 的導覽；實際 ${JSON.stringify(raws.map((r) => `+${r.at - t0}ms`))}）`);
    };
    const order0 = await reviewChildOrder(ctx);
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, '並排前（只有 README.md 可見，量中欄寬度）');
    await pressSplit(ctx, H);
    await expectSplit(ctx, { cols: [R, H], selected: H, shown: [R, H] }, `${R} 與 ${H} 並排`);
    await frameCheck('並排');
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [R, H], selected: 'LIVE', shown: ['LIVE'] }, '選 Live Output');
    await clickTab(ctx, H);
    await expectSplit(ctx, { cols: [R, H], selected: H, shown: [R, H] }, `點 ${H}（回到並排）`);
    await frameCheck('離開再回到並排');
    await pressSplit(ctx, L);
    await expectSplit(ctx, { cols: [R, H, L], selected: L, shown: [R, H, L] }, `按 ${L} 的並排鈕（三欄，${H} 移到中間）`);
    await frameCheck('三欄');
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [R, H, L], selected: R, shown: [R, H, L] }, `點 ${R}（換焦點欄）`);
    await pressSplit(ctx, H);
    await expectSplit(ctx, { cols: [R, L], selected: R, shown: [R, L] }, `按 ${H} 的並排鈕（移出）`);
    // 並排中點 page.html 會取代焦點欄（spec「替換」），所以先選 Live Output 離開並排，再點它單欄顯示。
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [R, L], selected: 'LIVE', shown: ['LIVE'] }, '選 Live Output');
    await clickTab(ctx, H);
    await expectSplit(ctx, { cols: [R, L], selected: H, shown: [H] }, `點 ${H}（單欄）`);
    await sleep(500);
    await frameCheck('移出並排後單欄顯示');
    await checkChildOrder(ctx, order0, '整段');
    noExceptions(ctx);
  });
}

// spec「整頁重畫不影響並排」與「切換 Project 不影響並排」共用的前置與斷言。README.md→long.md、docs/a.md→
// docs/design.md（兩欄都要能捲到中段；同 3.1「切到 Live Output 後整組恢復」的對應）。兩欄並排、焦點欄 docs/design.md，
// 兩欄都捲到中段，在面板與內容節點上貼記號、記下 #review 子節點順序。after() 之後斷言：仍為兩欄並排、焦點欄不變
// （expectSplit＋checkLayout）、兩欄捲動位置不變、內容沒有重新讀取、內容節點與面板節點都沒有被換掉、子節點順序不變。
async function splitSurvives(ctx, label, after) {
  const [L, D] = ['long.md', 'docs/design.md'];
  await need1280(ctx);
  await openTree(ctx);
  await openReady(ctx, L);
  await openReady(ctx, D);
  await clickTab(ctx, L);
  await expectSplit(ctx, { cols: [], selected: L, shown: [L] }, `前置：並排前（只有 ${L} 可見，量中欄寬度）`);
  await pressSplit(ctx, D);
  await expectSplit(ctx, { cols: [L, D], selected: D, shown: [L, D] }, `前置：${L} 與 ${D} 並排，焦點欄 ${D}`);
  const topL = await scrollHostTo(ctx, L, 0.4);
  const topD = await scrollHostTo(ctx, D, 0.5);
  need(await markHost(ctx, L, `${label}-L`), `在 ${L} 的內容節點貼記號`);
  need(await markHost(ctx, D, `${label}-D`), `在 ${D} 的內容節點貼記號`);
  need(await markPanels(ctx, [L, D], `${label}-panel`), `在 ${L} 與 ${D} 的面板上貼記號`);
  const order0 = await reviewChildOrder(ctx);
  const t0 = Date.now();
  await after();
  await expectSplit(ctx, { cols: [L, D], selected: D, shown: [L, D] }, `${label}後`);
  await checkColumnUntouched(ctx, L, t0, `${label}-L`, topL, label);
  await checkColumnUntouched(ctx, D, t0, `${label}-D`, topD, label);
  const lost = await unmarkedPanels(ctx, [L, D], `${label}-panel`);
  check(lost.length === 0, `${label}：面板節點沒有被換掉（記號不見的：${JSON.stringify(lost)}）`);
  await checkChildOrder(ctx, order0, label);
  noExceptions(ctx);
}

// spec「整頁重畫不影響並排」：投影每 100 ms 推送一份新的 version，持續 3 秒（數 #version 的 data-state-version 變化，
// 至少 10 次）。
async function segSplitRepaint() {
  await withCockpit('split-repaint', { windowSize: W1280, env: { COCKPIT_PREVIEW_PUSH_MS: '100' } }, async (ctx) => {
    await splitSurvives(ctx, '整頁重畫', async () => {
      let repaints = 0;
      let lastV = await stateVersion(ctx);
      const start = Date.now();
      while (Date.now() - start < 3000) {
        await sleep(100);
        const v = await stateVersion(ctx);
        if (v !== lastV) {
          repaints += 1;
          lastV = v;
        }
      }
      check(repaints >= 10, `3 秒內真的發生了多次整頁重畫（data-state-version 變化 ${repaints} 次）`);
    });
  });
}

// spec「切換 Project 不影響並排」：點左欄 Project 分頁中的另一個 Project（同 files-check「切換 Project 不影響分頁」的
// 做法：button[data-action="select-project"]，不是目前的那一個）。
async function segSplitProjectSwitch() {
  await withCockpit('split-project', { windowSize: W1280 }, async (ctx) => {
    await splitSurvives(ctx, '切換 Project', async () => {
      await switchLeftTab(ctx, 'Project');
      const other = await ctx.cdp.poll(() => {
        const items = Array.from(document.querySelectorAll('button[data-action="select-project"]')).filter((b) => window.__sc.visible(b));
        const o = items.find((b) => b.getAttribute('aria-current') !== 'true');
        return o ? o.getAttribute('data-project') : null;
      }, [], UI_TIMEOUT_MS);
      need(!!other, '左欄 Project 分頁中有另一個可見的 Project 項目');
      const sel = `button[data-action="select-project"][data-project="${other}"]`;
      need(await ctx.cdp.clickEl((s) => document.querySelector(s), [sel], `Project ${other}`), `點 Project ${other}`);
      const switched = await ctx.cdp.poll((s) => {
        const b = document.querySelector(s);
        return !!b && b.getAttribute('aria-current') === 'true';
      }, [sel], UI_TIMEOUT_MS);
      need(!!switched, `Project ${other} 成為目前 Project`);
      await sleep(500);
    });
  });
}

// ---------------------------------------------------------------------------
// file-split-view task 3.3：並排鈕、Ctrl＋點選、Ctrl＋Enter、欄位標記與無障礙（design D7；spec「檔案並排」的
// 「並排鈕」「加入」「焦點欄的標示與切換」）
// ---------------------------------------------------------------------------

// spec「Ctrl＋點選加入並排」：README.md 與 docs/a.md，目前為 README.md；按住 Ctrl 點選 docs/a.md 分頁 → 結果與按
// docs/a.md 的並排鈕相同（同 segSplitAdd 的斷言：兩欄並排、docs/a.md 為焦點欄與目前分頁、兩者 aria-pressed="true"、
// README.md 沒有重新讀取）。spec「並排鈕」：效果與按該分頁的並排鈕相同，所以再按住 Ctrl 點選 docs/a.md（並排組合中的
// 分頁）→ 移出，並排組合解除，README.md 單欄並為目前分頁。
// 另驗 clickEl 的 modifiers（本段第一次用到，當成新程式驗證）：頁面收到的 click 恰一次、落在 docs/a.md 分頁、ctrlKey 為 true。
async function segSplitCtrlClick() {
  const R = 'README.md';
  const A = 'docs/a.md';
  await withCockpit('split-ctrl-click', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, '按住 Ctrl 點選前');
    need(await markHost(ctx, R, 'ctrl-R'), `在 ${R} 的內容節點貼記號`);
    await ctx.cdp.run(() => window.__sc.installInputLog());
    await ctx.cdp.run(() => window.__sc.takeInput());
    const t0 = Date.now();
    await clickTab(ctx, A, { modifiers: MOD.CTRL, expectSelected: false });
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `按住 Ctrl 點選 ${A} 後`);
    const log = await ctx.cdp.run(() => window.__sc.takeInput());
    check(
      log.clicks.length === 1 && log.clicks[0].tab === A && log.clicks[0].ctrl === true,
      `clickEl 的 Ctrl 修飾鍵到達頁面：恰一次 click、落在 ${A} 分頁、ctrlKey=true（實際 ${JSON.stringify(log.clicks)}）`
    );
    await checkColumnUntouched(ctx, R, t0, 'ctrl-R', null, 'Ctrl＋點選加入並排');
    await clickTab(ctx, A, { modifiers: MOD.CTRL, expectSelected: false });
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, `再按住 Ctrl 點選並排組合中的 ${A} 後（等同按它的並排鈕：移出、解除）`);
    noExceptions(ctx);
  });
}

// spec「鍵盤加入並排」：README.md 與 docs/a.md，目前為 README.md，鍵盤焦點在分頁列的 README.md 上；按右方向鍵把焦點
// 移到 docs/a.md，再按 Ctrl＋Enter → 兩者並排，docs/a.md 為焦點欄。
// design D7：Ctrl＋Enter 必須 preventDefault()，否則 <button> 的 Enter 會再轉成一次 click、多跑一次一般選定。所以另驗
// Ctrl＋Enter 之後頁面沒有收到任何 click（只觸發一次動作），焦點仍在 docs/a.md 分頁上。
// 接著：再按 Ctrl＋Enter（並排組合中的分頁）→ 移出、解除；最後選 Live Output，方向鍵移到 README.md 按 Ctrl＋Enter →
// README.md 的並排鈕此時停用（目前分頁是 Live Output），等同一般選定：README.md 單欄並為目前分頁，同樣沒有多一次 click。
// pressKey 本段第一次用到（當成新程式驗證）：頁面收到的 keydown 的 key 與 ctrlKey 跟送出的相同。
async function segSplitKeyboard() {
  const R = 'README.md';
  const A = 'docs/a.md';
  const ctrlEnter = (ctx) => ctx.cdp.pressKey('Enter', 'Enter', 13, '\r', MOD.CTRL);
  const arrowRight = (ctx) => ctx.cdp.pressKey('ArrowRight', 'ArrowRight', 39, undefined, 0);
  const focusIs = async (ctx, name, label) => {
    const f = await ctx.cdp.poll((n) => window.__sc.focusName() === n, [name], UI_TIMEOUT_MS);
    check(!!f, `${label}：鍵盤焦點在 ${name}（實際 ${JSON.stringify(await ctx.cdp.run(() => window.__sc.focusName()))}）`);
  };
  await withCockpit('split-keyboard', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await clickTab(ctx, R);
    await focusIs(ctx, R, '點選 README.md 分頁後');
    await ctx.cdp.run(() => window.__sc.installInputLog());
    await ctx.cdp.run(() => window.__sc.takeInput());

    await arrowRight(ctx);
    await focusIs(ctx, A, '按右方向鍵後');
    const k1 = await ctx.cdp.run(() => window.__sc.takeInput());
    check(
      k1.keys.length === 1 && k1.keys[0].key === 'ArrowRight' && k1.keys[0].ctrl === false,
      `pressKey 送出的右方向鍵到達頁面：恰一個 keydown、key=ArrowRight、ctrlKey=false（實際 ${JSON.stringify(k1.keys)}）`
    );
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, '按右方向鍵後（只移動焦點，不選定）');

    await ctrlEnter(ctx);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `焦點在 ${A} 上按 Ctrl＋Enter 後`);
    await sleep(300);
    const k2 = await ctx.cdp.run(() => window.__sc.takeInput());
    check(
      k2.keys.length === 1 && k2.keys[0].key === 'Enter' && k2.keys[0].ctrl === true,
      `pressKey 送出的 Ctrl＋Enter 到達頁面：恰一個 keydown、key=Enter、ctrlKey=true（實際 ${JSON.stringify(k2.keys)}）`
    );
    check(k2.clicks.length === 0, `Ctrl＋Enter 只觸發一次動作：之後頁面沒有收到 click（實際 ${JSON.stringify(k2.clicks)}）`);
    await focusIs(ctx, A, 'Ctrl＋Enter 加入並排後');

    await ctrlEnter(ctx);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, `再對並排組合中的 ${A} 按 Ctrl＋Enter 後（移出、解除）`);
    await sleep(300);
    const k3 = await ctx.cdp.run(() => window.__sc.takeInput());
    check(k3.clicks.length === 0, `Ctrl＋Enter 移出時同樣沒有多一次 click（實際 ${JSON.stringify(k3.clicks)}）`);

    await clickTab(ctx, 'LIVE');
    await focusIs(ctx, 'LIVE', '點選 Live Output 分頁後');
    await ctx.cdp.run(() => window.__sc.takeInput());
    await arrowRight(ctx);
    await focusIs(ctx, R, '在 Live Output 上按右方向鍵後');
    await ctrlEnter(ctx);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, `目前為 Live Output、焦點在 ${R} 上按 Ctrl＋Enter 後（並排鈕停用，等同一般選定）`);
    await sleep(300);
    const k4 = await ctx.cdp.run(() => window.__sc.takeInput());
    check(k4.clicks.length === 0, `停用時的 Ctrl＋Enter 也沒有多一次 click（實際 ${JSON.stringify(k4.clicks)}）`);
    noExceptions(ctx);
  });
}

// spec「並排鈕停用時 Ctrl＋點選等同一般選定」：README.md 與 docs/a.md，目前為 docs/a.md，沒有並排組合；按住 Ctrl 點選
// docs/a.md 分頁 → 沒有形成並排，仍以單欄顯示 docs/a.md。前置另驗兩顆並排鈕的停用狀態：docs/a.md（目前分頁自己）停用、
// README.md 可用（目前分頁是它以外的檔案分頁）。接著選 Live Output，按住 Ctrl 點選 README.md（此時停用）→ 等同一般選定，
// README.md 單欄並為目前分頁。
async function segSplitDisabledCtrlClick() {
  const R = 'README.md';
  const A = 'docs/a.md';
  await withCockpit('split-disabled-ctrl', {}, async (ctx) => {
    const zh = await dictTexts(ctx, 'zh');
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await expectSplit(ctx, { cols: [], selected: A, shown: [A] }, '前置：目前為 docs/a.md、沒有並排組合');
    await expectDisabled(ctx, A, zh, `目前分頁就是 ${A}`);
    await expectEnabled(ctx, R, zh, `目前分頁是 ${R} 以外的檔案分頁`);
    await clickTab(ctx, A, { modifiers: MOD.CTRL, expectSelected: false });
    await sleep(500);
    await expectSplit(ctx, { cols: [], selected: A, shown: [A] }, `按住 Ctrl 點選 ${A}（並排鈕停用）後`);
    await clickTab(ctx, 'LIVE');
    await expectDisabled(ctx, R, zh, '目前分頁是 Live Output');
    await clickTab(ctx, R, { modifiers: MOD.CTRL });
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, `目前為 Live Output 時按住 Ctrl 點選 ${R}（並排鈕停用）後`);
    noExceptions(ctx);
  });
}

// spec「並排鈕」：並排鈕在分頁為目前分頁、或滑鼠移到分頁上時顯示；只有目前分頁的並排鈕在 Tab 順序中（design D7：
// 照關閉鈕的規則，目前分頁的並排鈕 tabindex="0"、其餘 -1）。README.md、docs/a.md、long.md，目前為 README.md：
//   1. 滑鼠移開：README.md 的並排鈕顯示，其餘不顯示；tabIndex 依序 0、-1、-1。Live Output 沒有並排鈕。
//   2. 滑鼠移到 docs/a.md 分頁上：它的並排鈕顯示（long.md 仍不顯示）；移開後又不顯示。
//   3. 按 docs/a.md 的並排鈕（兩欄並排，docs/a.md 為目前分頁）後滑鼠移開：docs/a.md 的顯示，README.md（並排組合中、
//      但不是目前分頁）不顯示；tabIndex 跟著改成 docs/a.md 為 0。滑鼠移到 README.md 上：它的顯示。
//   4. 真的按 Tab：焦點在 docs/a.md 分頁上按 Tab → 它的並排鈕；再按 Tab → 它的關閉鈕（pressKey('Tab') 第一次用到，
//      以焦點實際移動驗證）。
//   5. 選 Live Output、滑鼠移開：所有並排鈕都不顯示、tabIndex 都是 -1。
async function segSplitButtonShowAndTab() {
  const [R, A, L] = ['README.md', 'docs/a.md', 'long.md'];
  const files = [R, A, L];
  const expectUi = async (ctx, cur, shownSet, label) => {
    const u = await splitUi(ctx);
    for (const f of files) {
      const want = shownSet.includes(f);
      check(u[f] && u[f].shown === want, `${label}：${f} 的並排鈕${want ? '顯示' : '不顯示'}（shown=${u[f] ? u[f].shown : 'n/a'}、opacity=${u[f] ? u[f].opacity : 'n/a'}）`);
      const ti = f === cur ? 0 : -1;
      check(u[f] && u[f].tabIndex === ti, `${label}：${f} 的並排鈕 tabIndex=${ti}（實際 ${u[f] ? u[f].tabIndex : 'n/a'}）`);
    }
    check(u.LIVE && u.LIVE.buttons === 0, `${label}：Live Output 分頁沒有並排鈕（實際 ${u.LIVE ? u.LIVE.buttons : 'n/a'}）`);
    return u;
  };
  await withCockpit('split-show', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of files) await openReady(ctx, f);
    await clickTab(ctx, R);
    await mouseAway(ctx);
    await expectUi(ctx, R, [R], '目前為 README.md、滑鼠移開');
    await hoverTab(ctx, A);
    await expectUi(ctx, R, [R, A], `滑鼠移到 ${A} 分頁上`);
    await mouseAway(ctx);
    await expectUi(ctx, R, [R], `滑鼠從 ${A} 移開`);

    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `按 ${A} 的並排鈕後`);
    await mouseAway(ctx);
    await expectUi(ctx, A, [A], `並排中、目前為 ${A}、滑鼠移開`);
    await hoverTab(ctx, R);
    await expectUi(ctx, A, [A, R], `並排中滑鼠移到 ${R}（並排組合中、不是目前分頁）上`);
    await mouseAway(ctx);

    await clickTab(ctx, A);
    const f0 = await ctx.cdp.run(() => window.__sc.focusName());
    need(f0 === A, `點選 ${A} 分頁後鍵盤焦點在它上面（實際 ${JSON.stringify(f0)}）`);
    await ctx.cdp.pressKey('Tab', 'Tab', 9, undefined, 0);
    const f1 = await ctx.cdp.poll(() => window.__sc.focusName(), [], 1000);
    check(f1 === `split:${A}`, `在 ${A} 分頁上按 Tab → 焦點到它的並排鈕（實際 ${JSON.stringify(f1)}）`);
    await ctx.cdp.pressKey('Tab', 'Tab', 9, undefined, 0);
    const f2 = await ctx.cdp.poll(() => window.__sc.focusName(), [], 1000);
    check(f2 === `close:${A}`, `再按 Tab → 焦點到它的關閉鈕（實際 ${JSON.stringify(f2)}）`);

    await clickTab(ctx, 'LIVE');
    await mouseAway(ctx);
    const u = await splitUi(ctx);
    for (const f of files) {
      check(u[f].shown === false, `選定 Live Output、滑鼠移開：${f} 的並排鈕不顯示（shown=${u[f].shown}）`);
      check(u[f].tabIndex === -1, `選定 Live Output：${f} 的並排鈕 tabIndex=-1（實際 ${u[f].tabIndex}）`);
    }
    noExceptions(ctx);
  });
}

// spec「焦點欄的標示與切換」：並排組合中的每個分頁在分頁列上都帶欄位編號（1～3）標記，並有輔助技術讀得出的
// 「並排第 N 欄」說明（design D7：data-split-col＋CSS 數字徽章，說明以 aria-describedby 連到分頁按鈕）。
//   1. README.md、docs/a.md、long.md 三欄並排（焦點欄 docs/a.md），另開未並排的 note.txt：
//      - 並排組合中的分頁：data-split-col＝N、分頁按鈕的 ::after 徽章內容為 "N"、aria-describedby 依序指到
//        「並排第 N 欄」（字典 files.tab.splitCol）與完整路徑＋根目錄（同分頁的 title；修正第 1 輪，審查 Important：
//        不然兩個根目錄各自的 README.md 並排時分不出來）；無障礙樹上分頁的名稱仍是檔名（徽章不混進名稱）、說明為
//        「並排第 N 欄」接著路徑與根目錄。
//      - note.txt：沒有 data-split-col、沒有徽章（content: none）、沒有 aria-describedby，無障礙樹上的說明沒有「並排第 N 欄」
//   2. 選 Live Output（不在並排中，並排組合保留）：標記與說明照舊。
//   3. 點回 docs/a.md、按它的並排鈕移出 → 剩 README.md、long.md：long.md 改為第 2 欄（徽章與說明跟著改），docs/a.md 的
//      徽章與說明拿掉。
// axOf 第一次用到（當成新程式驗證）：先查 README.md 的關閉鈕，名稱必須是「關閉 README.md」、role 為 button。
async function segSplitColumnMarks() {
  const [R, A, L, N] = ['README.md', 'docs/a.md', 'long.md', 'note.txt'];
  const base = (p) => p.split('/').pop();
  await withCockpit('split-marks', {}, async (ctx) => {
    const zh = await dictTexts(ctx, 'zh');
    await ctx.cdp.send('DOM.enable');
    await ctx.cdp.send('Accessibility.enable');
    await openTree(ctx);
    for (const f of [R, A, L, N]) await openReady(ctx, f);
    const closeSel = `${await tabSelector(ctx, R)} ~ .review-tab-close`;
    const axClose = await axOf(ctx, closeSel);
    need(
      !axClose.error && axClose.role === 'button' && axClose.name === `關閉 ${R}`,
      `axOf 自我驗證：${R} 的關閉鈕在無障礙樹上是 button、名稱「關閉 ${R}」（實際 ${JSON.stringify(axClose)}）`
    );
    const expectMarks = async (members, label) => {
      const u = await splitUi(ctx);
      for (const f of [R, A, L, N]) {
        const n = members.indexOf(f) + 1;
        const ax = await axOf(ctx, await tabSelector(ctx, f));
        check(!ax.error && ax.role === 'tab' && ax.name === base(f), `${label}：${f} 在無障礙樹上是 tab、名稱為 ${JSON.stringify(base(f))}（徽章不混進名稱；實際 ${JSON.stringify(ax)}）`);
        if (n > 0) {
          const want = fill(zh.splitCol, { n });
          check(u[f].col === String(n), `${label}：${f} 的 data-split-col="${n}"（實際 ${JSON.stringify(u[f].col)}）`);
          check(new RegExp(`^"${n}"`).test(u[f].badge) && u[f].badgeDisplay !== 'none', `${label}：${f} 的分頁按鈕有數字徽章 "${n}"（::after content=${u[f].badge}、display=${u[f].badgeDisplay}）`);
          // 修正第 1 輪（審查 Important）：說明是「並排第 N 欄」加上完整路徑與根目錄（同分頁的 title），兩個根目錄各有
          // README.md 時才分得出是哪一個。
          const pathText = fill(zh.title, { path: f, root: ROOT_NAME });
          check(u[f].tabTitle === pathText, `${label}：${f} 的 title 為 ${JSON.stringify(pathText)}（實際 ${JSON.stringify(u[f].tabTitle)}）`);
          check(JSON.stringify(u[f].desc) === JSON.stringify([want, pathText]), `${label}：${f} 的 aria-describedby 依序指到「${want}」與路徑＋根目錄 ${JSON.stringify(pathText)}（實際 ${JSON.stringify(u[f].desc)}）`);
          const flat = (x) => x.replace(/\s+/g, ' ').trim();
          check(flat(ax.description) === flat(`${want} ${pathText}`), `${label}：${f} 在無障礙樹上的說明以「${want}」開頭、接著路徑與根目錄（實際 ${JSON.stringify(ax.description)}）`);
        } else {
          check(u[f].col === null, `${label}：${f} 不在並排組合，沒有 data-split-col（實際 ${JSON.stringify(u[f].col)}）`);
          check(u[f].badge === 'none' || u[f].badge === 'normal', `${label}：${f} 沒有數字徽章（::after content=${u[f].badge}）`);
          check(u[f].desc.length === 0, `${label}：${f} 沒有 aria-describedby（實際 ${JSON.stringify(u[f].desc)}）`);
          // 沒有 aria-describedby 時 Chrome 以分頁的 title（路徑與根目錄）當說明，所以只驗說明裡沒有任何「並排第 N 欄」。
          const colTexts = [1, 2, 3].map((k) => fill(zh.splitCol, { n: k }));
          check(!colTexts.some((c) => ax.description.includes(c)), `${label}：${f} 在無障礙樹上的說明沒有「並排第 N 欄」（實際 ${JSON.stringify(ax.description)}）`);
        }
      }
    };
    await buildThree(ctx, R, A, L, A);
    await expectMarks([R, A, L], '三欄並排（焦點欄 docs/a.md）');
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [R, A, L], selected: 'LIVE', shown: ['LIVE'] }, '選定 Live Output 後（並排組合保留）');
    await expectMarks([R, A, L], '選定 Live Output 期間');
    await clickTab(ctx, A);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, L], selected: L, shown: [R, L] }, `移出 ${A} 後`);
    await expectMarks([R, L], `移出 ${A} 後`);
    noExceptions(ctx);
  });
}

// spec「介面語言」（ui-language）＋task 3.3：並排鈕的 title（可用與停用兩種）、aria-label 與「並排第 N 欄」說明都來自
// i18n.js 字典，切換語言後跟著換。README.md 與 docs/a.md，目前為 docs/a.md：
//   1. 繁中：docs/a.md 的並排鈕停用、title 為繁中停用原因；README.md 的可用、title「並排」、aria-label「並排 README.md」。
//      按 README.md 的並排鈕 → docs/a.md、README.md 並排，說明分別為繁中「並排第 1 欄」「並排第 2 欄」，各自接著該語言
//      的路徑＋根目錄（files.tab.title；修正第 1 輪）。
//   2. 以頁面的 cockpitI18n.setLang('en')（頂列語言按鈕走的同一條路）切換成英文、頁面重新載入：分頁從 localStorage 還原後，
//      依當下狀態逐顆核對英文 title（可用／停用依 spec「加入」的停用條件推算）與英文 aria-label；並排組合必須已經還原
//      （task 3.6 的持久化；task 5.2 修正輪拿掉「沒還原就重新按」的退路，避免還原壞掉時被這段靜默補救），再核對英文說明。
//   兩種語言的五個字串都不相同（確認真的換了語言）。
async function segSplitI18n() {
  const R = 'README.md';
  const A = 'docs/a.md';
  const base = (p) => p.split('/').pop();
  const checkTitles = async (ctx, texts, label) => {
    const s = await splitState(ctx);
    const u = await splitUi(ctx);
    const cur = s.selected;
    for (const f of [R, A]) {
      const enabled = s.cols.length > 0 || (cur !== null && [R, A].includes(cur) && cur !== f);
      const want = enabled ? texts.split : texts.splitDisabled;
      check(u[f] && u[f].title === want, `${label}：${f} 的並排鈕（${enabled ? '可用' : '停用'}）title 為 ${JSON.stringify(want)}（實際 ${JSON.stringify(u[f] && u[f].title)}）`);
      check(u[f] && u[f].ariaDisabled === (enabled ? null : 'true'), `${label}：${f} 的並排鈕 aria-disabled 為 ${enabled ? '無' : '"true"'}（實際 ${JSON.stringify(u[f] && u[f].ariaDisabled)}）`);
      const label2 = fill(texts.splitNamed, { name: base(f) });
      check(u[f] && u[f].ariaLabel === label2, `${label}：${f} 的並排鈕 aria-label 為 ${JSON.stringify(label2)}（實際 ${JSON.stringify(u[f] && u[f].ariaLabel)}）`);
    }
  };
  const checkDesc = async (ctx, texts, cols, label) => {
    const u = await splitUi(ctx);
    cols.forEach((f, i) => {
      const want = fill(texts.splitCol, { n: i + 1 });
      const pathText = fill(texts.title, { path: f, root: ROOT_NAME });
      check(JSON.stringify(u[f].desc) === JSON.stringify([want, pathText]), `${label}：${f} 的說明為「${want}」與 ${JSON.stringify(pathText)}（實際 ${JSON.stringify(u[f].desc)}）`);
    });
  };
  await withCockpit('split-i18n', {}, async (ctx) => {
    const zh = await dictTexts(ctx, 'zh');
    const en = await dictTexts(ctx, 'en');
    for (const k of Object.keys(SPLIT_TEXT_KEYS)) check(zh[k] !== en[k], `${SPLIT_TEXT_KEYS[k]} 的中英文不同（zh ${JSON.stringify(zh[k])}、en ${JSON.stringify(en[k])}）`);
    check(/另一個檔案分頁/.test(zh.splitDisabled), `繁中停用原因說明需要先選另一個檔案分頁（實際 ${JSON.stringify(zh.splitDisabled)}）`);
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await checkTitles(ctx, zh, '繁中、目前為 docs/a.md');
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [A, R], selected: R, shown: [A, R] }, `繁中：按 ${R} 的並排鈕後`);
    await checkTitles(ctx, zh, '繁中、並排中');
    await checkDesc(ctx, zh, [A, R], '繁中');

    const lang0 = await ctx.cdp.run(() => document.documentElement.lang);
    need(lang0 === 'zh-Hant', `切換前 <html lang> 為 zh-Hant（實際 ${JSON.stringify(lang0)}）`);
    await ctx.cdp.run(() => {
      setTimeout(() => window.cockpitI18n.setLang('en'), 0);
      return true;
    });
    const switched = await ctx.cdp.poll(() => document.documentElement.lang === 'en' && !!window.__sc, [], 10000);
    need(!!switched, '切換成英文後頁面重新載入、<html lang> 為 en');
    await waitForFirstProjection(ctx.cdp, ctx.preview.port);
    const restored = await ctx.cdp.poll((a, b) => !!window.__sc.fileTab(a) && !!window.__sc.fileTab(b), [R, A], UI_TIMEOUT_MS);
    need(!!restored, `重新載入後 ${R} 與 ${A} 兩個分頁都還原`);
    await sleep(300);
    await checkTitles(ctx, en, '英文、重新載入後');
    const s = await splitState(ctx);
    need(JSON.stringify(s.cols) === JSON.stringify([A, R]), `英文、重新載入後：並排組合已還原為 ${JSON.stringify([A, R])}（task 3.6；實際 ${JSON.stringify(s.cols)}）`);
    await checkDesc(ctx, en, s.cols, '英文');
    noExceptions(ctx);
  });
}

// 審查第 1 項（file-split-view task 3.3）：closeTab() 的並排分支要跟 toggleSplit() 一樣呼叫 revealTab()，讓接手的分頁
// 捲進分頁列的視野。視窗 1280×900、依序打開 10 個檔案讓分頁列需要橫向捲動；目前分頁為分頁列右段、但不是最後一個的
// report.pdf 時按 README.md（最左側）的並排鈕 → 並排組合 report.pdf、README.md，README.md 為焦點欄。前置確認 report.pdf
// 此時在可視範圍外。關閉 README.md → 焦點欄移出、沒有右側欄改為左側欄 report.pdf，並排解除，report.pdf 為目前分頁；
// 它的包裝元素（含並排鈕與關閉鈕）整個在分頁列的可視範圍內。接手的分頁刻意不選最後一個：最後一個分頁被捲到時分頁列
// 已捲到底，不管有沒有 revealTab() 都整個看得到。
// 注意：這一段在未修的程式上也是綠的（2026-10-05 實測）。用滑鼠按關閉鈕時焦點在被關分頁的包裝元素裡，closeTab() 把焦點
// 移給接手分頁，Chrome 的 focus() 自己就把它捲到可視範圍中間。revealTab() 是焦點不在被關分頁時的保險，真實輸入觸發不到
// 那條路，所以本段只當回歸防護，不當作 revealTab() 的紅綠證據。
async function segSplitCloseReveals() {
  const R = 'README.md';
  const M = 'report.pdf';
  const files = [R, 'docs/a.md', 'docs/design.md', 'docs/pic.png', 'long.md', 'note.txt', 'page.html', M, 'style.css', 'src/main.rs'];
  await withCockpit('split-close-reveal', { windowSize: W1280 }, async (ctx) => {
    await need1280(ctx);
    await openTree(ctx);
    for (const f of files) await openFile(ctx, f);
    const v0 = await ctx.cdp.run((n) => window.__sc.tabInView(n), M);
    need(!!v0 && v0.overflow > 200, `分頁列需要橫向捲動（可捲 ${v0 ? v0.overflow : '?'} px）`);
    await clickTab(ctx, M);
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [M, R], selected: R, shown: [M, R] }, `目前為 ${M} 時按 ${R} 的並排鈕後`);
    await mouseAway(ctx);
    const v1 = await ctx.cdp.run((n) => window.__sc.tabInView(n), M);
    need(!!v1 && !v1.inView, `前置：${M} 此時不在分頁列的可視範圍內（${JSON.stringify(v1)}）`);
    await pressClose(ctx, R);
    await expectSplit(ctx, { cols: [], selected: M, shown: [M] }, `關閉 ${R} 後`);
    await sleep(200);
    const v2 = await ctx.cdp.run((n) => window.__sc.tabInView(n), M);
    check(!!v2 && v2.inView, `關閉 ${R} 後，接手的 ${M} 整個捲進分頁列的可視範圍（含並排鈕與關閉鈕；${JSON.stringify(v2)}）`);
    noExceptions(ctx);
  });
}

// 修正第 1 輪（file-split-view task 3.3；審查 Minor 1）：不在並排中（目前分頁是 Live Output）時移出並排組合的成員，目前分頁
// 不變（spec「移出」：移出前不是並排中，目前分頁不變），所以分頁列不該被捲回目前分頁。視窗 1280×900、開 10 個檔案讓分頁列
// 要橫向捲動；page.html、report.pdf、style.css 三欄並排後選 Live Output（分頁列捲回最左）。
//   1. 按 page.html 的並排鈕（點擊會先把它捲進可視範圍）→ 移出；之後 scrollLeft 與點擊前相同（≤ 1 px），目前分頁仍是 Live Output。
//   2. 按 report.pdf 的關閉鈕 → 並排解除；scrollLeft 同樣不變，目前分頁仍是 Live Output；鍵盤焦點移到右側相鄰的 style.css
//      （同「不在並排組合中的分頁」被關閉時的規則；移到 Live Output 會讓 focus() 把分頁列捲回最左）。
// report.pdf 右側還有兩個分頁，關閉它之後 scrollLeft 不會因可捲範圍縮小而被夾住。
async function segSplitAwayNoScroll() {
  const [P, Q, S] = ['page.html', 'report.pdf', 'style.css'];
  const files = ['README.md', 'docs/a.md', 'docs/design.md', 'docs/pic.png', 'long.md', 'note.txt', P, Q, S, 'src/main.rs'];
  const scrollLeft = (ctx) => ctx.cdp.run(() => document.querySelector('#review [role="tablist"]').scrollLeft);
  await withCockpit('split-away-noscroll', { windowSize: W1280 }, async (ctx) => {
    await need1280(ctx);
    await openTree(ctx);
    for (const f of files) await openFile(ctx, f);
    await clickTab(ctx, P);
    await pressSplit(ctx, Q);
    await pressSplit(ctx, S);
    await expectSplit(ctx, { cols: [P, Q, S], selected: S, shown: [P, Q, S] }, `前置：${P}、${Q}、${S} 三欄並排`);
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [P, Q, S], selected: 'LIVE', shown: ['LIVE'] }, '前置：選定 Live Output（並排組合保留）');

    // 點擊工具會先把按鈕捲進可視範圍；先自己捲好（同樣的 scrollIntoView），再記下基準，點擊時就不會再捲。
    const scrollToButton = async (rel, cls) => {
      await ctx.cdp.run((p, c) => {
        window.__sc.fileTab(p).parentElement.querySelector(c).scrollIntoView({ block: 'nearest', inline: 'nearest' });
        return true;
      }, rel, cls);
      await sleep(150);
      return scrollLeft(ctx);
    };

    // 1. 按並排鈕移出
    const s1 = await scrollToButton(P, '.review-tab-split');
    need(s1 > 0, `前置：${P} 的並排鈕捲進可視範圍時分頁列已捲離最左（scrollLeft ${s1}），「捲回目前分頁」才觀察得到`);
    await pressSplit(ctx, P);
    await expectSplit(ctx, { cols: [Q, S], selected: 'LIVE', shown: ['LIVE'] }, `不在並排中按 ${P} 的並排鈕後`);
    await sleep(300);
    const s1b = await scrollLeft(ctx);
    check(Math.abs(s1b - s1) <= 1, `移出 ${P} 後分頁列沒有被捲動（scrollLeft 前 ${s1}、後 ${s1b}）`);

    // 2. 關閉並排成員
    const before = await scrollToButton(Q, '.review-tab-close');
    need(before > 0, `前置：${Q} 的關閉鈕捲進可視範圍時分頁列已捲離最左（scrollLeft ${before}）`);
    await pressClose(ctx, Q);
    await expectSplit(ctx, { cols: [], selected: 'LIVE', shown: ['LIVE'] }, `不在並排中關閉 ${Q} 後（只剩 ${S}，並排解除）`);
    await sleep(300);
    const after = await scrollLeft(ctx);
    check(Math.abs(after - before) <= 1, `關閉 ${Q} 後分頁列沒有被捲動（scrollLeft 前 ${before}、後 ${after}）`);
    const f = await ctx.cdp.run(() => window.__sc.focusName());
    check(f === S, `關閉 ${Q} 後鍵盤焦點移到右側相鄰的 ${S}（實際 ${JSON.stringify(f)}）`);
    noExceptions(ctx);
  });
}

// --- file-split-view task 3.4：在欄內按下滑鼠切換焦點欄（design D6；spec「焦點欄的標示與切換」）---

// fn(...args) 回傳的元素上，找一個「按下去不會觸發其他動作」的座標：命中測試落在該元素（或其子孫）上，而且不在連結、
// 按鈕、輸入欄位或分頁上。依序試元素框內的幾個比例位置，回傳第一個合格的 { x, y, hit }；元素不存在或沒有合格的點時
// 回傳 { x: null, ... } 附診斷。不像 clickEl() 先 scrollIntoView：「捲動位置不變」的斷言不能被點擊工具自己捲動干擾。
async function freePoint(ctx, fn, args) {
  return ctx.cdp.eval(`(() => {
    const el = (${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')});
    if (!el) return { x: null, why: 'no-element' };
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return { x: null, why: 'zero-size' };
    const tried = [];
    for (const fy of [0.5, 0.3, 0.7, 0.15, 0.85]) {
      for (const fx of [0.5, 0.25, 0.75, 0.1, 0.9]) {
        const x = r.left + r.width * fx;
        const y = r.top + r.height * fy;
        if (x < 0 || y < 0 || x >= innerWidth || y >= innerHeight) continue;
        const h = document.elementFromPoint(x, y);
        const tag = h ? h.tagName.toLowerCase() : null;
        if (h && (h === el || el.contains(h)) && !h.closest('a, button, input, select, textarea, [role="tab"]')) return { x, y, hit: tag };
        tried.push(tag);
      }
    }
    return { x: null, why: 'no-free-point', tried };
  })()`);
}

// 在 (x, y) 按下並放開滑鼠左鍵（真的滑鼠事件，不捲動）。
async function pressAt(ctx, x, y) {
  const base = { x, y, button: 'left', clickCount: 1 };
  await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
  await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
  await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
}

// 在某個檔案分頁的內容（檢視器捲動容器 .file-viewer-host）上按一下，不捲動、不按在連結上。
async function pressInHost(ctx, rel) {
  const pt = await freePoint(ctx, (p) => {
    const panel = window.__sc.panelOf(window.__sc.fileTab(p));
    return panel ? panel.querySelector('.file-viewer-host') : null;
  }, [rel]);
  need(pt && pt.x !== null, `${rel} 的內容上找得到可以按的位置（不在連結、按鈕上；${JSON.stringify(pt)}）`);
  await pressAt(ctx, pt.x, pt.y);
}

// 用滑鼠點完之後沒有 :focus-visible 外框（專案 memory programmatic-focus-after-pointer-counts-as-focus-visible：滑鼠操作後
// 程式呼叫 focus() 會被 Chrome 判成 :focus-visible）。整份文件裡沒有任何元素符合 :focus-visible，附 activeElement 診斷。
async function checkNoFocusVisible(ctx, label) {
  const f = await ctx.cdp.run(() => {
    const desc = (el) => (el ? `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ''}${typeof el.className === 'string' && el.className.trim() ? `.${el.className.trim().replace(/\s+/g, '.')}` : ''}` : null);
    return { fv: desc(document.querySelector(':focus-visible')), active: desc(document.activeElement), hasFocus: document.hasFocus() };
  });
  check(f.fv === null, `${label}：沒有元素處於 :focus-visible（實際 ${JSON.stringify(f.fv)}；activeElement ${JSON.stringify(f.active)}、document.hasFocus() ${f.hasFocus}）`);
}

// spec「在欄內點選切換焦點欄」（README.md→long.md、docs/a.md→docs/design.md：兩欄都要能捲到中段）：long.md（第 1 欄）與
// docs/design.md（第 2 欄、焦點欄）並排，兩欄都捲到中段並貼記號。
//   1. 在 long.md 欄的內容上按一下（不在連結上、不捲動）→ long.md 成為焦點欄與目前分頁；兩欄都沒有重新讀取、內容節點沒換、
//      捲動位置不變；沒有 :focus-visible 外框，等一次整頁重畫（推送間隔 1 秒）之後仍沒有。
//   2. 分頁列上的 pointerdown 不切換（design D6 只看並排面板）：在 docs/design.md 的分頁上只按下不放開，目前分頁仍是 long.md；
//      放開後的 click 照一般選定，docs/design.md 成為焦點欄。
//   3. 不在並排中不切換：選 Live Output，在 Live Output 面板上按一下，目前分頁仍是 Live Output，並排組合不變。
//   4. 自我驗證 :focus-visible 的量法：按 Tab 之後，確實有元素符合 :focus-visible（量法不是恆為空）。
async function segSplitPointerFocus() {
  const [L, D] = ['long.md', 'docs/design.md'];
  await withCockpit('split-pointer', { env: { COCKPIT_PREVIEW_PUSH_MS: '1000' } }, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, L);
    await openReady(ctx, D);
    await clickTab(ctx, L);
    await pressSplit(ctx, D);
    await expectSplit(ctx, { cols: [L, D], selected: D, shown: [L, D] }, `前置：${L} 與 ${D} 並排，${D} 為焦點欄`);
    const topL = await scrollHostTo(ctx, L, 0.4);
    const topD = await scrollHostTo(ctx, D, 0.5);
    need(await markHost(ctx, L, 'ptr-L'), `在 ${L} 的內容節點貼記號`);
    need(await markHost(ctx, D, 'ptr-D'), `在 ${D} 的內容節點貼記號`);
    const t0 = Date.now();
    await pressInHost(ctx, L);
    await expectSplit(ctx, { cols: [L, D], selected: L, shown: [L, D] }, `在 ${L} 欄的內容上按一下後`);
    await sleep(500);
    await checkColumnUntouched(ctx, L, t0, 'ptr-L', topL, '在欄內點選切換焦點欄');
    await checkColumnUntouched(ctx, D, t0, 'ptr-D', topD, '在欄內點選切換焦點欄');
    await checkNoFocusVisible(ctx, `在 ${L} 欄按一下後`);
    const v0 = await stateVersion(ctx);
    const v1 = await waitForRepaint(ctx, v0, 5000);
    need(v1 !== null, `之後發生整頁重畫（data-state-version ${v0} → ${v1}）`);
    await sleep(200);
    await checkNoFocusVisible(ctx, `在 ${L} 欄按一下、整頁重畫之後`);
    await expectSplit(ctx, { cols: [L, D], selected: L, shown: [L, D] }, '整頁重畫之後');

    // 2. 分頁列上的 pointerdown 不切換
    const tabPt = await ctx.cdp.run((p) => {
      const t = window.__sc.fileTab(p);
      const r = t.getBoundingClientRect();
      const x = r.left + r.width / 2;
      const y = r.top + r.height / 2;
      const h = document.elementFromPoint(x, y);
      return { x, y, onTab: !!h && (h === t || t.contains(h)) };
    }, D);
    need(tabPt.onTab, `${D} 分頁的中心點得到它（${JSON.stringify(tabPt)}）`);
    await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: tabPt.x, y: tabPt.y });
    await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: tabPt.x, y: tabPt.y, button: 'left', clickCount: 1 });
    await sleep(300);
    const mid = await splitState(ctx);
    check(mid.selected === L, `在分頁列上的 ${D} 分頁只按下、還沒放開時，目前分頁仍是 ${L}（實際 ${JSON.stringify(mid.selected)}）`);
    await ctx.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: tabPt.x, y: tabPt.y, button: 'left', clickCount: 1 });
    await expectSplit(ctx, { cols: [L, D], selected: D, shown: [L, D] }, `放開後（click 照一般選定，${D} 為焦點欄）`);

    // 3. 不在並排中（Live Output）不切換
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [L, D], selected: 'LIVE', shown: ['LIVE'] }, '選 Live Output');
    const livePt = await freePoint(ctx, () => document.getElementById('review-panel-live'), []);
    need(livePt && livePt.x !== null, `Live Output 面板上找得到可以按的位置（${JSON.stringify(livePt)}）`);
    await pressAt(ctx, livePt.x, livePt.y);
    await sleep(300);
    await expectSplit(ctx, { cols: [L, D], selected: 'LIVE', shown: ['LIVE'] }, '在 Live Output 面板上按一下後');

    // 4. 自我驗證量法
    await ctx.cdp.pressKey('Tab', 'Tab', 9, '');
    await sleep(150);
    const fv = await ctx.cdp.run(() => !!document.querySelector(':focus-visible'));
    check(fv, '自我驗證：按 Tab 之後有元素符合 :focus-visible（checkNoFocusVisible 的量法不是恆為空）');
    noExceptions(ctx);
  });
}

// spec「替換」的 md 相對連結入口＋design D6（「在哪一欄點的連結，就在哪一欄開」）：README.md（第 1 欄）與 docs/a.md
// （第 2 欄、焦點欄）並排；在 README.md 欄點 [設計](docs/design.md#決策) → pointerdown 先讓 README.md 欄成為焦點欄，
// 接著的 click 開檔替換它：並排組合依序 docs/design.md、docs/a.md，docs/design.md 為焦點欄；README.md 分頁仍在；
// docs/a.md 欄沒有重新讀取；沒有 :focus-visible 外框。
async function segSplitLinkInNonFocus() {
  const [R, A, D] = ['README.md', 'docs/a.md', 'docs/design.md'];
  await withCockpit('split-link-nonfocus', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await clickTab(ctx, R);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `前置：${R} 與 ${A} 並排，${A} 為焦點欄`);
    need(await markHost(ctx, A, 'link-A'), `在 ${A} 的內容節點貼記號`);
    const t0 = Date.now();
    const linkSel = `a[data-md-path="${D}"]`;
    need(
      await ctx.cdp.clickEl((p, sel) => window.__sc.panelOf(window.__sc.fileTab(p)).querySelector(sel), [R, linkSel], `${R} 欄（非焦點欄）的 md 相對連結 ${linkSel}`),
      `在非焦點的 ${R} 欄點 md 相對連結（${D}#決策）`
    );
    const s = await expectSplit(ctx, { cols: [D, A], selected: D, shown: [D, A] }, `在非焦點欄點 md 相對連結後（${D} 在 ${R} 那一欄開啟）`);
    check(s.names.includes(R), `被取代的 ${R} 分頁仍在分頁列（實際 ${JSON.stringify(s.names)}）`);
    await waitFileReady(ctx, D);
    await checkColumnUntouched(ctx, A, t0, 'link-A', null, '在非焦點欄點 md 相對連結');
    await checkNoFocusVisible(ctx, '點 md 相對連結後');
    noExceptions(ctx);
  });
}

// design Risks「pointerdown 改焦點欄可能與既有點擊行為衝突」：report.pdf（第 1 欄）與 README.md（第 2 欄、焦點欄）並排；
// 在 report.pdf 欄的工具列點「下一頁」→ report.pdf 成為焦點欄，頁碼由「1 / 3」變「2 / 3」（翻頁照常生效）；兩欄都沒有
// 重新讀取；沒有 :focus-visible 外框。
async function segSplitPdfNextInNonFocus() {
  const [P, R] = ['report.pdf', 'README.md'];
  const statusOf = (ctx) =>
    ctx.cdp.run((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const s = panel && panel.querySelector('[data-viewer="pdf"] .pdf-page-status');
      return s ? s.textContent : null;
    }, P);
  const nextButton = (p) => {
    const panel = window.__sc.panelOf(window.__sc.fileTab(p));
    const dicts = window.cockpitI18n.dictionaries;
    const names = [dicts.zh['viewers.pdf.next'], dicts.en['viewers.pdf.next']];
    return panel ? Array.from(panel.querySelectorAll('.pdf-toolbar button')).find((b) => names.includes(b.textContent)) || null : null;
  };
  await withCockpit('split-pdf-next', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, R);
    await openFile(ctx, P);
    const pdf0 = await waitPdfFit(ctx, P, null, '單欄時');
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [P, R], selected: R, shown: [P, R] }, `前置：${P} 與 ${R} 並排，${R} 為焦點欄`);
    await waitPdfFit(ctx, P, pdf0.pageWidth, '並排後');
    // 同 nextButton()（poll 只把傳入的函式本身送進頁面，不能引用外層的 nextButton）。
    const ready = await ctx.cdp.poll((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const dicts = window.cockpitI18n.dictionaries;
      const names = [dicts.zh['viewers.pdf.next'], dicts.en['viewers.pdf.next']];
      const b = panel ? Array.from(panel.querySelectorAll('.pdf-toolbar button')).find((x) => names.includes(x.textContent)) : null;
      return !!b && b.getAttribute('aria-disabled') !== 'true';
    }, [P], UI_TIMEOUT_MS);
    need(!!ready, `${P} 欄的「下一頁」按鈕存在且可用`);
    const st0 = await statusOf(ctx);
    need(st0 === '1 / 3', `前置：${P} 在第 1 頁（頁碼 ${JSON.stringify(st0)}）`);
    need(await markHost(ctx, R, 'pdf-R'), `在 ${R} 的內容節點貼記號`);
    need(await markHost(ctx, P, 'pdf-P'), `在 ${P} 的內容節點貼記號`);
    const t0 = Date.now();
    need(await ctx.cdp.clickEl(nextButton, [P], `${P} 欄（非焦點欄）的「下一頁」`), `在非焦點的 ${P} 欄點「下一頁」`);
    await expectSplit(ctx, { cols: [P, R], selected: P, shown: [P, R] }, `在 ${P} 欄點「下一頁」後`);
    const turned = await ctx.cdp.poll((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const s = panel && panel.querySelector('[data-viewer="pdf"] .pdf-page-status');
      return !!s && s.textContent === '2 / 3';
    }, [P], UI_TIMEOUT_MS);
    check(!!turned, `${P} 翻到第 2 頁（頁碼「2 / 3」；實際 ${JSON.stringify(await statusOf(ctx))}）`);
    await sleep(300);
    await checkColumnUntouched(ctx, R, t0, 'pdf-R', null, '在非焦點的 PDF 欄點「下一頁」');
    await checkColumnUntouched(ctx, P, t0, 'pdf-P', null, '在非焦點的 PDF 欄點「下一頁」');
    await checkNoFocusVisible(ctx, '點「下一頁」後');
    noExceptions(ctx);
  });
}

// html 檢視器的內容在 sandbox iframe 裡（不允許腳本），iframe 內的 pointerdown 不會傳到父文件；按進 iframe 時父文件的
// window 收到 blur、activeElement 變成那個 <iframe>（task 3.4 實測）。控制端裁決（task 3.4 修正第 1 輪）：以 window 的
// blur 補上，spec 不改。page.html（第 1 欄）與 README.md（第 2 欄、焦點欄）並排，在 page.html 的 iframe 元素上貼記號、
// 掛 load 計數，另在父文件記 window blur 次數（只計 target 為 window 的）：
//   1. 在非焦點的 page.html 欄的 iframe 上按一下 → page.html 成為焦點欄；兩欄都沒有重新讀取（README.md 內容節點沒換、
//      沒有 page.html 的 raw 請求）；iframe 仍是同一個節點、沒有重新載入；window blur 確實發生；沒有 :focus-visible。
//   2. 在 README.md 欄的內容上按一下（焦點欄回到 README.md），再按 page.html 的 iframe → 再次切換（可重複）。
//   3. 焦點欄自己的 iframe：點 page.html 分頁（焦點回到父文件），再按它的 iframe → window blur 有發生，狀態不變。
//   4. window 失焦但 activeElement 不是 iframe（切到別的應用程式的情況，以合成的 window blur 事件模擬）：點 README.md 分頁，
//      對 window 送一個 blur 事件 → 狀態不變。
//   5. 在 page.html 欄的工具列（iframe 之外、不在按鈕上）按一下 → page.html 成為焦點欄；沒有 :focus-visible。
//   6. 不在並排中：按 page.html 的並排鈕移出（並排解除，剩 README.md），點 page.html 分頁單欄顯示，再按它的 iframe →
//      window blur 有發生，仍是單欄、目前分頁 page.html、沒有並排組合。
async function segSplitHtmlColumnPress() {
  const [H, R] = ['page.html', 'README.md'];
  const iframeOf = (p) => window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('iframe');
  await withCockpit('split-html-press', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, R);
    await openFile(ctx, H);
    const ready = await ctx.cdp.poll((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const view = panel && panel.querySelector('[data-viewer="html"]');
      const f = view && !view.hidden ? view.querySelector('iframe') : null;
      return !!f && panel.querySelectorAll('iframe').length === 1;
    }, [H], 10000);
    need(!!ready, `${H} 的 html 檢視器已顯示 iframe`);
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [H, R], selected: R, shown: [H, R] }, `前置：${H} 與 ${R} 並排，${R} 為焦點欄`);
    await sleep(500);
    need(
      await ctx.cdp.run((p) => {
        const f = window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('iframe');
        f.__scFrame = 'html-press';
        window.__scFrameLoads = 0;
        f.addEventListener('load', () => {
          window.__scFrameLoads += 1;
        });
        window.__scBlurs = 0;
        window.addEventListener('blur', (e) => {
          if (e.target === window) window.__scBlurs += 1;
        });
        return true;
      }, H),
      `在 ${H} 的 iframe 上貼記號、掛 load 計數，並記 window blur 次數`
    );
    const blurs = () => ctx.cdp.run(() => window.__scBlurs);
    const pressFrame = async (label) => {
      const pt = await freePoint(ctx, iframeOf, [H]);
      need(pt && pt.x !== null && pt.hit === 'iframe', `${label}：${H} 的 iframe 上找得到可以按的位置（命中 iframe 元素本身；${JSON.stringify(pt)}）`);
      const b0 = await blurs();
      await pressAt(ctx, pt.x, pt.y);
      await sleep(300);
      const m = await ctx.cdp.run(() => ({ active: document.activeElement ? document.activeElement.tagName.toLowerCase() : null, blurs: window.__scBlurs }));
      check(m.blurs > b0, `${label}：按 iframe 之後父文件的 window 收到 blur（前 ${b0}、後 ${m.blurs}；activeElement ${JSON.stringify(m.active)}）`);
    };
    need(await markHost(ctx, R, 'html-R'), `在 ${R} 的內容節點貼記號`);
    const t0 = Date.now();

    // 1. 非焦點欄的 iframe
    await pressFrame('在非焦點欄的 iframe 上按一下');
    await expectSplit(ctx, { cols: [H, R], selected: H, shown: [H, R] }, `在非焦點的 ${H} 欄的 iframe 上按一下後`);
    await sleep(300);
    await checkColumnUntouched(ctx, R, t0, 'html-R', null, '在非焦點欄的 iframe 上按一下');
    const f = await ctx.cdp.run((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const frames = panel ? panel.querySelectorAll('iframe') : [];
      const fr = frames[0] || null;
      return { count: frames.length, marked: !!fr && fr.__scFrame === 'html-press', connected: !!fr && fr.isConnected, loads: window.__scFrameLoads };
    }, H);
    check(f.count === 1 && f.marked && f.connected, `在非焦點欄的 iframe 上按一下：${H} 的 iframe 仍是同一個節點（${JSON.stringify(f)}）`);
    check(f.loads === 0, `在非焦點欄的 iframe 上按一下：${H} 的 iframe 沒有重新載入（load 事件 ${f.loads} 次）`);
    const raws = contentReqsSince(ctx, H, t0);
    check(raws.length === 0, `在非焦點欄的 iframe 上按一下：${H} 沒有重新讀取內容（之後的 render／raw 請求 ${JSON.stringify(raws.map((r) => `${r.kind}@+${r.at - t0}ms`))}）`);
    await checkNoFocusVisible(ctx, `在 ${H} 欄的 iframe 上按一下後`);

    // 2. 可重複
    await pressInHost(ctx, R);
    await expectSplit(ctx, { cols: [H, R], selected: R, shown: [H, R] }, `在 ${R} 欄的內容上按一下後`);
    await pressFrame('再按一次非焦點欄的 iframe');
    await expectSplit(ctx, { cols: [H, R], selected: H, shown: [H, R] }, `再在 ${H} 欄的 iframe 上按一下後`);

    // 3. 焦點欄自己的 iframe
    await clickTab(ctx, H);
    await expectSplit(ctx, { cols: [H, R], selected: H, shown: [H, R] }, `點 ${H} 分頁（焦點回到父文件）`);
    await pressFrame('按焦點欄自己的 iframe');
    await sleep(200);
    await expectSplit(ctx, { cols: [H, R], selected: H, shown: [H, R] }, `按焦點欄 ${H} 自己的 iframe 後（不變）`);

    // 4. window 失焦但 activeElement 不是 iframe
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [H, R], selected: R, shown: [H, R] }, `點 ${R} 分頁`);
    const syn = await ctx.cdp.run(() => {
      const before = window.__scBlurs;
      window.dispatchEvent(new FocusEvent('blur'));
      return { active: document.activeElement ? document.activeElement.tagName.toLowerCase() : null, fired: window.__scBlurs - before };
    });
    need(syn.fired === 1 && syn.active !== 'iframe', `對 window 送出合成的 blur 事件（activeElement ${JSON.stringify(syn.active)}，blur 計數 +${syn.fired}）`);
    await sleep(300);
    await expectSplit(ctx, { cols: [H, R], selected: R, shown: [H, R] }, 'window 失焦、activeElement 不是 iframe 時（不變）');

    // 5. 工具列
    const barPt = await freePoint(ctx, (p) => window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('.file-toolbar'), [H]);
    need(barPt && barPt.x !== null, `${H} 欄的工具列上找得到可以按的位置（不在按鈕上；${JSON.stringify(barPt)}）`);
    await pressAt(ctx, barPt.x, barPt.y);
    await expectSplit(ctx, { cols: [H, R], selected: H, shown: [H, R] }, `在 ${H} 欄的工具列（iframe 之外）按一下後`);
    await checkNoFocusVisible(ctx, `在 ${H} 欄的工具列按一下後`);

    // 6. 不在並排中
    await pressSplit(ctx, H);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, `按 ${H} 的並排鈕移出（並排解除，剩 ${R}）`);
    await clickTab(ctx, H);
    await expectSplit(ctx, { cols: [], selected: H, shown: [H] }, `點 ${H} 分頁（單欄）`);
    const back = await ctx.cdp.poll((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const view = panel && panel.querySelector('[data-viewer="html"]');
      const fr = view && !view.hidden ? view.querySelector('iframe') : null;
      return !!fr && fr.getBoundingClientRect().width > 0;
    }, [H], 10000);
    need(!!back, `${H} 單欄顯示時 iframe 可見`);
    await pressFrame('單欄時按 iframe');
    await sleep(200);
    await expectSplit(ctx, { cols: [], selected: H, shown: [H] }, `單欄時按 ${H} 的 iframe 後（不變）`);
    noExceptions(ctx);
  });
}

// 在頁面裡記下尚未執行（或尚未清掉）的 setTimeout／setInterval，以 callback 的函式名稱計數（file-split-view task 3.4 修正
// 第 2 輪：驗「焦點離開 iframe 後沒有殘留的輪詢計時器」）。包在 window.setTimeout／clearTimeout／setInterval／clearInterval
// 上，不改產品程式；files.js 每次呼叫時才讀全域的 setTimeout，所以頁面載入之後才包也攔得到。
// window.__scTimers.pending(name)：名稱為 name、還沒執行也沒被清掉的 timeout，加上還沒被清掉的 interval 的個數。
const installTimerLog = (ctx) =>
  ctx.cdp.run(() => {
    if (window.__scTimers) return true;
    const pending = new Map(); // id → callback 名稱
    const st = window.setTimeout;
    const ct = window.clearTimeout;
    const si = window.setInterval;
    const ci = window.clearInterval;
    window.setTimeout = function (fn, ms, ...rest) {
      if (typeof fn !== 'function') return st.call(window, fn, ms, ...rest);
      let id = null;
      id = st.call(
        window,
        function () {
          pending.delete(id);
          return fn.apply(this, arguments);
        },
        ms,
        ...rest
      );
      pending.set(id, fn.name);
      return id;
    };
    window.clearTimeout = function (id) {
      pending.delete(id);
      return ct.call(window, id);
    };
    window.setInterval = function (fn, ms, ...rest) {
      const id = si.call(window, fn, ms, ...rest);
      if (typeof fn === 'function') pending.set(id, fn.name);
      return id;
    };
    window.clearInterval = function (id) {
      pending.delete(id);
      return ci.call(window, id);
    };
    window.__scTimers = { pending: (name) => [...pending.values()].filter((n) => n === name).length };
    return true;
  });

// 第二個 html 檔（只寫進 ui_preview 的暫存副本，不改 cockpit/examples/fixtures/；比照三欄段的 long.txt）。內容夠高、有段落，
// 不含腳本（反正 sandbox 不允許）。
const PAGE2_HTML = 'page2.html';
const PAGE2_BODY = '<!doctype html>\n<html lang="zh-Hant">\n<head><meta charset="utf-8" /><title>page2.html（split-check）</title></head>\n<body>\n<h1>page2.html</h1>\n<p>split-check：兩個 html 欄之間直接切換焦點欄（file-split-view task 3.4 修正第 2 輪）。</p>\n</body>\n</html>\n';
// 輪詢計時器的 callback 名稱（files.js 的 followIframeFocus；改名時這裡一起改，下面的正向對照會先抓到）。
const IFRAME_POLL_FN = 'followIframeFocus';

// 審查 Important（task 3.4 修正第 2 輪；控制端裁決 (a) 輪詢）：焦點從一個 iframe 直接移到另一個 iframe 時，父文件收不到
// blur／focus／focusin／focusout，只有 document.activeElement 默默改變。page.html（第 1 欄）與暫存副本裡的 page2.html
// （第 2 欄、焦點欄）並排，兩個 iframe 都貼記號、掛 load 計數，頁面裡記下以函式名稱計的待執行計時器（installTimerLog）：
//   0. 基準：焦點還在父文件時，沒有輪詢計時器（平常不得有計時器在跑）。
//   1. 按 page.html 的 iframe → page.html 成為焦點欄；焦點在 iframe 裡時有一個輪詢計時器（正向對照：量法量得到）。
//   2. 直接按 page2.html 的 iframe → page2.html 成為焦點欄；兩欄都沒有 render／raw 請求；兩個 iframe 都是同一個節點、
//      沒有重新載入。
//   3. 在 page2.html 欄的工具列（父文件、不在按鈕上）按一下，焦點離開 iframe → 0.6 秒後沒有輪詢計時器，焦點欄不變。
async function segSplitTwoHtmlColumns() {
  const [H1, H2] = ['page.html', PAGE2_HTML];
  const iframeReady = (ctx, rel) =>
    ctx.cdp.poll((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const view = panel && panel.querySelector('[data-viewer="html"]');
      const f = view && !view.hidden ? view.querySelector('iframe') : null;
      return !!f && panel.querySelectorAll('iframe').length === 1;
    }, [rel], 10000);
  await withCockpit('split-two-html', {}, async (ctx) => {
    fs.writeFileSync(path.join(ctx.preview.reviewRepo, H2), PAGE2_BODY);
    need(await installTimerLog(ctx), '在頁面裡包好 setTimeout／setInterval，記下待執行的計時器');
    await openTree(ctx);
    await openFile(ctx, H1);
    need(!!(await iframeReady(ctx, H1)), `${H1} 的 html 檢視器已顯示 iframe`);
    await openFile(ctx, H2);
    need(!!(await iframeReady(ctx, H2)), `${H2} 的 html 檢視器已顯示 iframe`);
    await clickTab(ctx, H1);
    await pressSplit(ctx, H2);
    await expectSplit(ctx, { cols: [H1, H2], selected: H2, shown: [H1, H2] }, `前置：${H1} 與 ${H2} 並排，${H2} 為焦點欄`);
    await sleep(500);
    need(
      await ctx.cdp.run((names) => {
        window.__scFrameLoads = {};
        for (const p of names) {
          const f = window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('iframe');
          f.__scFrame = `two-${p}`;
          window.__scFrameLoads[p] = 0;
          f.addEventListener('load', () => {
            window.__scFrameLoads[p] += 1;
          });
        }
        return true;
      }, [H1, H2]),
      '在兩個 iframe 上貼記號、掛 load 計數'
    );
    const pollTimers = () => ctx.cdp.run((n) => window.__scTimers.pending(n), IFRAME_POLL_FN);
    const activeTag = () => ctx.cdp.run(() => (document.activeElement ? document.activeElement.tagName.toLowerCase() : null));
    const pressFrame = async (rel, label) => {
      const pt = await freePoint(ctx, (p) => window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('iframe'), [rel]);
      need(pt && pt.x !== null && pt.hit === 'iframe', `${label}：${rel} 的 iframe 上找得到可以按的位置（${JSON.stringify(pt)}）`);
      await pressAt(ctx, pt.x, pt.y);
      await sleep(600); // 輪詢週期之內的反應時間（files.js 約 200 ms）＋餘裕
    };
    const t0 = Date.now();

    // 0. 基準
    const base = await pollTimers();
    check(base === 0, `焦點還在父文件時沒有輪詢計時器（${IFRAME_POLL_FN} 待執行 ${base} 個；activeElement ${JSON.stringify(await activeTag())}）`);

    // 1. 按 page.html 的 iframe
    await pressFrame(H1, `按 ${H1} 的 iframe`);
    await expectSplit(ctx, { cols: [H1, H2], selected: H1, shown: [H1, H2] }, `按 ${H1} 的 iframe 後`);
    const during = await pollTimers();
    check(during === 1, `焦點在 ${H1} 的 iframe 裡時恰有一個輪詢計時器（${IFRAME_POLL_FN} 待執行 ${during} 個；activeElement ${JSON.stringify(await activeTag())}）`);

    // 2. 直接按 page2.html 的 iframe
    await pressFrame(H2, `直接按 ${H2} 的 iframe`);
    await expectSplit(ctx, { cols: [H1, H2], selected: H2, shown: [H1, H2] }, `從 ${H1} 的 iframe 直接按進 ${H2} 的 iframe 後`);
    const fr = await ctx.cdp.run((names) => {
      const out = {};
      for (const p of names) {
        const panel = window.__sc.panelOf(window.__sc.fileTab(p));
        const frames = panel ? panel.querySelectorAll('iframe') : [];
        const f = frames[0] || null;
        out[p] = { count: frames.length, marked: !!f && f.__scFrame === `two-${p}`, connected: !!f && f.isConnected, loads: window.__scFrameLoads[p] };
      }
      return out;
    }, [H1, H2]);
    for (const p of [H1, H2]) {
      check(fr[p].count === 1 && fr[p].marked && fr[p].connected, `兩個 html 欄之間切換：${p} 的 iframe 仍是同一個節點（${JSON.stringify(fr[p])}）`);
      check(fr[p].loads === 0, `兩個 html 欄之間切換：${p} 的 iframe 沒有重新載入（load 事件 ${fr[p].loads} 次）`);
      const reads = contentReqsSince(ctx, p, t0);
      check(reads.length === 0, `兩個 html 欄之間切換：${p} 沒有重新讀取內容（之後的 render／raw 請求 ${JSON.stringify(reads.map((r) => `${r.kind}@+${r.at - t0}ms`))}）`);
    }

    // 3. 焦點離開 iframe
    const barPt = await freePoint(ctx, (p) => window.__sc.panelOf(window.__sc.fileTab(p)).querySelector('.file-toolbar'), [H2]);
    need(barPt && barPt.x !== null, `${H2} 欄的工具列上找得到可以按的位置（不在按鈕上；${JSON.stringify(barPt)}）`);
    await pressAt(ctx, barPt.x, barPt.y);
    await sleep(600);
    const tag = await activeTag();
    need(tag !== 'iframe', `在 ${H2} 欄的工具列按一下後焦點離開 iframe（activeElement ${JSON.stringify(tag)}）`);
    const after = await pollTimers();
    check(after === 0, `焦點離開 iframe 後沒有殘留的輪詢計時器（${IFRAME_POLL_FN} 待執行 ${after} 個）`);
    await expectSplit(ctx, { cols: [H1, H2], selected: H2, shown: [H1, H2] }, `焦點離開 iframe 後（焦點欄不變）`);
    noExceptions(ctx);
  });
}

// ---------------------------------------------------------------------------
// 窄視窗（file-split-view task 3.5；design D5；spec「檔案並排」的「窄視窗」、scenario「窄視窗只顯示焦點欄」
// 「窄視窗下仍替換焦點欄」，以及「自動更新」的 scenario「只查詢可見的檔案分頁」）
//
// 寬度以 CDP Emulation.setDeviceMetricsOverride 在 1280 與 700 之間切換（同 need1280()）。700 < 760（style.css 的單欄
// 斷點，也是 files.js 判斷並排顯示的寬度門檻），1280 ≥ 760。中欄寬度隨視窗改變，所以 checkLayout() 的「中欄寬度不變」
// 基準改成每種寬度各自一份（setWidth() 切換時換基準）。
// ---------------------------------------------------------------------------

const NARROW_W = 700;
const WIDE_W = 1280;
// spec「只查詢可見的檔案分頁」的觀察時間（「觀察 10 秒」）。
const SPEC_OBSERVE_MS = 10000;
// 10 秒窗內可見分頁至少被查幾次：相鄰兩次查詢相隔 2 秒加一次回應時間，10 秒窗理論上 4～5 次；取 3 留邊界（同 baseline
// 段 6 秒窗取 2 的理由）。這條只防「可見的分頁沒在輪詢」，主斷言是不可見的分頁 0 次。
const SPEC_MIN_HITS = 3;

// 把視窗寬度改成 w（高 900），確認 innerWidth 與 matchMedia("(min-width: 760px)") 都跟著變，並換 checkLayout() 的
// 中欄寬度基準。等 300 ms 讓 matchMedia 的 change 事件與版面更新跑完（之後的斷言仍各自輪詢到狀態相符）。
async function setWidth(ctx, w) {
  if (!ctx.baseByWidth) ctx.baseByWidth = {};
  if (ctx.curWidth !== undefined) ctx.baseByWidth[ctx.curWidth] = ctx.baseReviewWidth;
  const r = await ctx.cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: 900, deviceScaleFactor: 1, mobile: false });
  need(!r.error, `Emulation.setDeviceMetricsOverride ${w}×900（${r.error ? JSON.stringify(r.error) : 'ok'}）`);
  await sleep(300);
  const m = await ctx.cdp.run(() => ({ w: window.innerWidth, wide: window.matchMedia('(min-width: 760px)').matches }));
  need(m.w === w, `視窗寬度為 ${w}（innerWidth 實際 ${m.w}）`);
  need(m.wide === w >= 760, `(min-width: 760px) ${w >= 760 ? '成立' : '不成立'}（實際 ${m.wide}）`);
  ctx.curWidth = w;
  ctx.baseReviewWidth = ctx.baseByWidth[w];
}

// 斷言顯示的欄數＝實際可見的分頁內容（tabpanel）的數目。
async function checkShownColumns(ctx, expected, label) {
  const s = await reviewState(ctx);
  const shown = s.tabs.filter((t) => t.visible).map((t) => t.name);
  check(shown.length === expected.length, `${label}：顯示的欄數為 ${expected.length}（實際 ${shown.length}：${JSON.stringify(shown)}）`);
}

// 單欄版面（< 760 寬）整頁捲動，底部有固定的狀態列（.region-statusbar）。clickEl() 的 scrollIntoView({ block: 'nearest' })
// 會把頁面下方的元素捲到視窗最底，剛好被狀態列蓋住，命中測試失敗（task 3.5 實測：分頁列落在 y≈870～900，命中的是
// .region-statusbar）。所以在窄視窗下點分頁或按欄內容之前，先把目標捲到視窗中間（捲的是頁面，不是檢視器容器本身）。
async function revealCenter(ctx, fn, args, desc) {
  const ok = await ctx.cdp.eval(`(() => { const el = (${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')});
    if (!el) return false; el.scrollIntoView({ block: 'center', inline: 'nearest' }); return true; })()`);
  need(ok, `找得到${desc}（捲到視窗中間用）`);
  await sleep(150);
}
const revealHost = (ctx, rel) =>
  revealCenter(ctx, (p) => {
    const panel = window.__sc.panelOf(window.__sc.fileTab(p));
    return panel ? panel.querySelector('.file-viewer-host') : null;
  }, [rel], ` ${rel} 的檢視器容器`);
const revealTab = (ctx, name) => revealCenter(ctx, (n) => window.__sc.tabByName(n), [name], `分頁 ${name}`);

// 某個檔案分頁的面板在畫面上點不點得到：hidden、尺寸為 0，或中心點命中的不是它。
const panelHittable = (ctx, rel) =>
  ctx.cdp.run((p) => {
    const panel = window.__sc.panelOf(window.__sc.fileTab(p));
    if (!panel) return { exists: false };
    const r = panel.getBoundingClientRect();
    const x = r.left + r.width / 2;
    const y = r.top + r.height / 2;
    const h = r.width > 0 && r.height > 0 ? document.elementFromPoint(x, y) : null;
    return { exists: true, hidden: panel.hidden, w: r.width, h: r.height, hit: !!h && panel.contains(h) };
  }, rel);

// spec「窄視窗只顯示焦點欄」：1280 寬時 README.md（a）與 docs/a.md（b）並排、焦點欄為 docs/a.md；改為 700 → 只顯示
// docs/a.md、頁面沒有橫向捲軸、#review 沒有 data-split、並排組合與分頁列標記不變；改回 1280 → 恢復兩欄並排。再來回
// 一次，確認不是只有第一次有效。顯示的欄數另以 checkShownColumns() 明確斷言。
async function segNarrowFocusOnly() {
  const [R, A] = ['README.md', 'docs/a.md'];
  await withCockpit('narrow-focus', { windowSize: W1280 }, async (ctx) => {
    await setWidth(ctx, WIDE_W);
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await clickTab(ctx, R);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `前置：寬 ${WIDE_W}，${R} 與 ${A} 並排，${A} 為焦點欄`);
    await checkShownColumns(ctx, [R, A], `寬 ${WIDE_W}`);
    for (const round of [1, 2]) {
      await setWidth(ctx, NARROW_W);
      await expectSplit(ctx, { cols: [R, A], selected: A, shown: [A] }, `第 ${round} 次改為寬 ${NARROW_W}（只顯示焦點欄）`);
      await checkShownColumns(ctx, [A], `第 ${round} 次寬 ${NARROW_W}`);
      await setWidth(ctx, WIDE_W);
      await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `第 ${round} 次改回寬 ${WIDE_W}（恢復兩欄並排）`);
      await checkShownColumns(ctx, [R, A], `第 ${round} 次改回寬 ${WIDE_W}`);
    }
    noExceptions(ctx);
  });
}

// spec「自動更新」的 scenario「只查詢可見的檔案分頁」（a＝README.md、b＝docs/a.md、c＝note.txt）＋「從不可見變成可見時
// 立即查詢一次」「不可見的檔案分頁（含窄視窗下未顯示的並排欄）不得查詢」「分頁變成不可見之後才回來的舊回應必須丟棄」。
// 量的是頁面實際發出、送往服務的中繼資料查詢（CDP Network），不是 CSS。
//   1. 寬 1280，README.md 與 docs/a.md 並排、焦點欄 docs/a.md，note.txt 已打開但不可見。觀察 10 秒：只有 README.md 與
//      docs/a.md 被查（各至少 3 次），note.txt 0 次。
//   2. 以 Fetch 攔住 README.md 的中繼資料查詢，等一筆卡在半路，改為寬 700：卡住的那筆在 1 秒內被頁面中止；之後放行時
//      Chrome 回報已不存在。關掉攔截。
//   3. 寬 700 觀察 10 秒：只有 docs/a.md 被查（至少 3 次），README.md 與 note.txt 都 0 次；README.md 沒有過期標示（舊回應
//      沒有套用）。
//   4. 改回寬 1280：README.md 在 1 秒內被查一次（立即查詢，不等 2 秒的輪詢間隔）。再觀察 10 秒：README.md 與 docs/a.md
//      各至少 3 次，note.txt 0 次。
async function segNarrowQueriesVisibleOnly() {
  const [R, A, N] = ['README.md', 'docs/a.md', 'note.txt'];
  await withCockpit('narrow-poll', { windowSize: W1280 }, async (ctx) => {
    await setWidth(ctx, WIDE_W);
    await openTree(ctx);
    for (const f of [R, A, N]) await openReady(ctx, f);
    await clickTab(ctx, R);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `前置：寬 ${WIDE_W}，${R} 與 ${A} 並排，${A} 為焦點欄，${N} 已打開`);

    const seen1 = await observeMeta(ctx, SPEC_OBSERVE_MS);
    log(`寬 ${WIDE_W}：${seen1.windowMs} ms 內的中繼資料查詢 ${JSON.stringify(seen1.byPath)}`);
    for (const f of [R, A]) check((seen1.byPath[f] || 0) >= SPEC_MIN_HITS, `寬 ${WIDE_W}：並排中可見的 ${f} 在 ${SPEC_OBSERVE_MS} ms 內被查詢至少 ${SPEC_MIN_HITS} 次（實際 ${seen1.byPath[f] || 0}）`);
    check((seen1.byPath[N] || 0) === 0, `寬 ${WIDE_W}：不可見的 ${N} 沒有被查詢（實際 ${seen1.byPath[N] || 0}）`);
    const strays1 = Object.keys(seen1.byPath).filter((p) => p !== R && p !== A);
    check(strays1.length === 0, `寬 ${WIDE_W}：窗內只有 ${R} 與 ${A} 的中繼資料查詢（其他路徑：${JSON.stringify(strays1)}）`);

    const hold = await installMetaHold(ctx, R);
    await waitFirstHeld(hold, R);
    const tNarrow = Date.now();
    await setWidth(ctx, NARROW_W);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [A] }, `改為寬 ${NARROW_W}`);
    const r = await waitCanceled(ctx, hold.held[0].networkId, 1000);
    check(
      !!r && r.canceled,
      `改為寬 ${NARROW_W} 後 1 秒內，未顯示的 ${R} 卡住的查詢被頁面中止（實際 ${JSON.stringify(r ? { canceled: r.canceled, errorText: r.errorText, after: r.failedAt - tNarrow } : null)}）`
    );
    await hold.release(STALE_RESPONSE.status, STALE_RESPONSE.body);
    hold.held.forEach((h, i) => check(h.outcome === 'gone', `攔下的第 ${i + 1} 筆 ${R} 查詢放行時 Chrome 回報已不存在（實際 ${h.outcome}${h.error ? `：${h.error}` : ''}）`));
    const dis = await ctx.cdp.send('Fetch.disable');
    need(!dis.error, `關掉 Fetch 攔截（${dis.error ? JSON.stringify(dis.error) : 'ok'}）`);

    const seen2 = await observeMeta(ctx, SPEC_OBSERVE_MS);
    log(`寬 ${NARROW_W}：${seen2.windowMs} ms 內的中繼資料查詢 ${JSON.stringify(seen2.byPath)}`);
    check((seen2.byPath[A] || 0) >= SPEC_MIN_HITS, `寬 ${NARROW_W}：焦點欄 ${A} 在 ${SPEC_OBSERVE_MS} ms 內被查詢至少 ${SPEC_MIN_HITS} 次（實際 ${seen2.byPath[A] || 0}）`);
    check((seen2.byPath[R] || 0) === 0, `寬 ${NARROW_W}：未顯示的並排欄 ${R} 沒有被查詢（實際 ${seen2.byPath[R] || 0}）`);
    check((seen2.byPath[N] || 0) === 0, `寬 ${NARROW_W}：不可見的 ${N} 沒有被查詢（實際 ${seen2.byPath[N] || 0}）`);
    const strays2 = Object.keys(seen2.byPath).filter((p) => p !== A);
    check(strays2.length === 0, `寬 ${NARROW_W}：窗內只有 ${A} 的中繼資料查詢（其他路徑：${JSON.stringify(strays2)}）`);
    const rSnap = await fileSnap(ctx, R);
    check(
      !!rSnap && !rSnap.stale && !rSnap.statusShown,
      `寬 ${NARROW_W}：${R} 沒有過期標示、狀態列隱藏（放行的舊回應沒有套用；實際 ${JSON.stringify(rSnap && { stale: rSnap.stale, status: rSnap.statusShown ? rSnap.statusText : null })}）`
    );

    const tWide = Date.now();
    await setWidth(ctx, WIDE_W);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `改回寬 ${WIDE_W}`);
    const firstR = await (async () => {
      const start = Date.now();
      while (Date.now() - start < POLL_INTERVAL_MS + UI_TIMEOUT_MS) {
        const hit = ctx.net.reqs.find((x) => x.metaPath === R && x.at >= tWide);
        if (hit) return hit;
        await sleep(20);
      }
      return null;
    })();
    check(
      !!firstR && firstR.at - tWide <= 1000,
      `改回寬 ${WIDE_W} 後 ${R} 立即查詢一次（1 秒內，不等 ${POLL_INTERVAL_MS} ms 的輪詢間隔；實際 ${firstR ? `+${firstR.at - tWide}ms` : '沒有'}）`
    );
    const seen3 = await observeMeta(ctx, SPEC_OBSERVE_MS);
    log(`改回寬 ${WIDE_W}：${seen3.windowMs} ms 內的中繼資料查詢 ${JSON.stringify(seen3.byPath)}`);
    for (const f of [R, A]) check((seen3.byPath[f] || 0) >= SPEC_MIN_HITS, `改回寬 ${WIDE_W}：${f} 在 ${SPEC_OBSERVE_MS} ms 內被查詢至少 ${SPEC_MIN_HITS} 次（實際 ${seen3.byPath[f] || 0}）`);
    check((seen3.byPath[N] || 0) === 0, `改回寬 ${WIDE_W}：${N} 沒有被查詢（實際 ${seen3.byPath[N] || 0}）`);
    const everN = ctx.net.reqs.filter((x) => x.metaPath === N && x.at >= tNarrow);
    check(everN.length === 0, `改寬度之後 ${N} 始終沒有任何中繼資料查詢（實際 ${everN.length} 筆）`);
    noExceptions(ctx);
  });
}

// spec「窄視窗下仍替換焦點欄」（a＝README.md、b＝docs/a.md、c＝note.txt）：寬 700，README.md 與 docs/a.md 並排、焦點欄
// docs/a.md，另已打開 note.txt。點選 note.txt 分頁 → 寬 700 時只顯示 note.txt，並排組合依序 README.md、note.txt；改為
// 1280 → README.md、note.txt 兩欄並排，note.txt 為焦點欄。並排組合在寬 1280 時組好再改成 700（GIVEN 是 700 寬時的狀態）。
async function segNarrowReplaceFocus() {
  const [R, A, N] = ['README.md', 'docs/a.md', 'note.txt'];
  await withCockpit('narrow-replace', { windowSize: W1280 }, async (ctx) => {
    await setWidth(ctx, WIDE_W);
    await openTree(ctx);
    for (const f of [R, A, N]) await openReady(ctx, f);
    await clickTab(ctx, R);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `前置：寬 ${WIDE_W}，${R} 與 ${A} 並排`);
    await setWidth(ctx, NARROW_W);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [A] }, `前置：寬 ${NARROW_W}，只顯示焦點欄 ${A}`);
    await revealTab(ctx, N);
    await clickTab(ctx, N);
    await expectSplit(ctx, { cols: [R, N], selected: N, shown: [N] }, `寬 ${NARROW_W} 時點選 ${N}（取代焦點欄，只顯示 ${N}）`);
    await checkShownColumns(ctx, [N], `寬 ${NARROW_W} 點選 ${N} 後`);
    await setWidth(ctx, WIDE_W);
    await expectSplit(ctx, { cols: [R, N], selected: N, shown: [R, N] }, `改為寬 ${WIDE_W}（依序 ${R}、${N}，${N} 為焦點欄）`);
    await checkShownColumns(ctx, [R, N], `改為寬 ${WIDE_W} 後`);
    noExceptions(ctx);
  });
}

// design D6 在窄視窗下的邊界（task 3.4 審查交給 3.5）：寬 700 只剩焦點欄時，沒有任何非焦點欄可以點，按下滑鼠不改焦點欄；
// 從寬變窄、再變寬之後，在非焦點欄內按下滑鼠切換焦點欄仍然正常。README.md（第 1 欄）與 docs/a.md（第 2 欄）。
//   第 1 輪：寬 1280、焦點欄 docs/a.md。改為 700：README.md 的面板 hidden、點不到；在 docs/a.md（焦點欄）的內容上按一下 →
//   焦點欄與並排組合不變、仍只顯示 docs/a.md。改回 1280，在 README.md 欄的內容上按一下 → README.md 成為焦點欄。
//   第 2 輪：同上，兩欄角色對調（焦點欄 README.md，最後按 docs/a.md 欄 → docs/a.md 成為焦點欄）。
async function segNarrowPointerFocus() {
  const [R, A] = ['README.md', 'docs/a.md'];
  await withCockpit('narrow-pointer', { windowSize: W1280 }, async (ctx) => {
    await setWidth(ctx, WIDE_W);
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, A);
    await clickTab(ctx, R);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `前置：寬 ${WIDE_W}，${R} 與 ${A} 並排，${A} 為焦點欄`);
    for (const [focus, other] of [
      [A, R],
      [R, A],
    ]) {
      await setWidth(ctx, NARROW_W);
      await expectSplit(ctx, { cols: [R, A], selected: focus, shown: [focus] }, `寬 ${NARROW_W}，焦點欄 ${focus}`);
      const hit = await panelHittable(ctx, other);
      check(hit.exists && hit.hidden === true && !hit.hit, `寬 ${NARROW_W}：非焦點欄 ${other} 的面板 hidden、點不到（${JSON.stringify(hit)}）`);
      await revealHost(ctx, focus);
      await pressInHost(ctx, focus);
      await sleep(300);
      await expectSplit(ctx, { cols: [R, A], selected: focus, shown: [focus] }, `寬 ${NARROW_W} 時在焦點欄 ${focus} 的內容上按一下後（不變）`);
      await setWidth(ctx, WIDE_W);
      await expectSplit(ctx, { cols: [R, A], selected: focus, shown: [R, A] }, `改回寬 ${WIDE_W}`);
      await pressInHost(ctx, other);
      await expectSplit(ctx, { cols: [R, A], selected: other, shown: [R, A] }, `改回寬 ${WIDE_W} 後在 ${other} 欄的內容上按一下（切換焦點欄）`);
    }
    noExceptions(ctx);
  });
}

// ---------------------------------------------------------------------------
// 持久化並排組合（file-split-view task 3.6；spec「分頁還原」；design D8、Migration Plan）
//
// 鍵 cockpit.fileTabs 維持 v: 2，多兩個可選欄位：split（並排組合）、splitFocus（焦點欄），都是「儲存的 tabs 陣列」中的
// 索引；沒有並排組合時兩個欄位都不寫。spec scenario 的檔名對應同 3.1：a.md→README.md、b.md→docs/a.md；「還原並排」的
// docs/b.md 不在 fixture 裡，改用 long.md。
// 種資料的方式：先經畫面打開分頁，讓前端寫出真的 v2 紀錄（含 root_id），讀回來改寫 tabs／split／splitFocus／current
// 後寫回，再重新載入頁面。這樣不必另外查根目錄的 API，每一筆分頁紀錄都是前端自己寫出的形狀。
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'cockpit.fileTabs';

const readStored = (ctx) =>
  ctx.cdp.run((k) => {
    const raw = localStorage.getItem(k);
    return raw === null ? null : JSON.parse(raw);
  }, STORAGE_KEY);

const writeStored = (ctx, data) =>
  ctx.cdp.run(
    (k, v) => {
      localStorage.setItem(k, v);
      return localStorage.getItem(k) === v;
    },
    STORAGE_KEY,
    JSON.stringify(data)
  );

// 一筆分頁紀錄的名稱（同頁面端的 tabName()）：Git Graph 為 'GRAPH'，檔案（含 v1 沒有 kind 的紀錄）為相對路徑。
const storedName = (e) => (e && e.kind === 'graph' ? 'GRAPH' : e && typeof e.path === 'string' ? e.path : JSON.stringify(e));
const hasKey = (o, k) => !!o && Object.prototype.hasOwnProperty.call(o, k);

// 讀出前端寫的紀錄並斷言寫入格式（design D8）：v 仍為 2、tabs 依序為 exp.names；exp.split（名稱陣列，依欄位順序）換算成
// exp.names 中的索引後等於 split，splitFocus 為 exp.focus 的索引；exp.split 為 null 時 split 與 splitFocus 兩個欄位都不存在
// （不是 null，也不是空陣列）。exp.current：目前分頁名稱（'LIVE'＝null）。點擊處理比 CDP 回覆晚幾毫秒，所以先輪詢到相符
// 或逾時，再逐項斷言。回傳讀到的紀錄。
async function expectStored(ctx, exp, label) {
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const wantSplit = exp.split === null ? null : exp.split.map((n) => exp.names.indexOf(n));
  const wantFocus = exp.split === null ? null : exp.names.indexOf(exp.focus);
  const curOk = (d) => (exp.current === 'LIVE' ? d.current === null : !!d.current && storedName(d.current) === exp.current);
  const splitOk = (d) => (exp.split === null ? !hasKey(d, 'split') && !hasKey(d, 'splitFocus') : same(d.split, wantSplit) && d.splitFocus === wantFocus);
  const matches = (d) => !!d && d.v === 2 && Array.isArray(d.tabs) && same(d.tabs.map(storedName), exp.names) && splitOk(d) && curOk(d);
  let d = null;
  const start = Date.now();
  for (;;) {
    d = await readStored(ctx).catch(() => null);
    if (matches(d) || Date.now() - start >= UI_TIMEOUT_MS) break;
    await sleep(100);
  }
  need(!!d && typeof d === 'object', `${label}：localStorage 的 ${STORAGE_KEY} 有紀錄（實際 ${JSON.stringify(d)}）`);
  check(d.v === 2, `${label}：版號仍為 v: 2（實際 ${JSON.stringify(d.v)}）`);
  const names = Array.isArray(d.tabs) ? d.tabs.map(storedName) : null;
  check(same(names, exp.names), `${label}：tabs 依序為 ${JSON.stringify(exp.names)}（實際 ${JSON.stringify(names)}）`);
  if (exp.split === null) {
    check(!hasKey(d, 'split'), `${label}：沒有並排組合時不寫 split 欄位（實際 ${JSON.stringify(d.split)}）`);
    check(!hasKey(d, 'splitFocus'), `${label}：沒有並排組合時不寫 splitFocus 欄位（實際 ${JSON.stringify(d.splitFocus)}）`);
  } else {
    check(same(d.split, wantSplit), `${label}：split 為儲存的 tabs 中的索引 ${JSON.stringify(wantSplit)}（＝${JSON.stringify(exp.split)}；實際 ${JSON.stringify(d.split)}）`);
    check(d.splitFocus === wantFocus, `${label}：splitFocus 為 ${exp.focus} 在儲存的 tabs 中的索引 ${wantFocus}（實際 ${JSON.stringify(d.splitFocus)}）`);
  }
  check(curOk(d), `${label}：current 為 ${exp.current === 'LIVE' ? 'null（Live Output）' : exp.current}（實際 ${JSON.stringify(d.current)}）`);
  return d;
}

// 重新載入頁面並等分頁還原：先在舊頁面貼記號，等到新的 document（記號消失、頁面工具已重新安裝）與第一份真投影，再等
// 分頁列依序為 Live Output 加 names。回傳按下重新載入的時間（之後的 console 才算這次載入的）。
async function reloadPage(ctx, names, label) {
  need(
    await ctx.cdp.run(() => {
      window.__scOldDoc = true;
      return true;
    }),
    `${label}：在重新載入前的頁面貼記號`
  );
  const at = Date.now();
  const r = await ctx.cdp.send('Page.reload', { ignoreCache: false });
  need(!r.error, `${label}：Page.reload（${r.error ? JSON.stringify(r.error) : 'ok'}）`);
  const fresh = await ctx.cdp.poll(() => !!window.__sc && window.__scOldDoc !== true, [], 10000);
  need(!!fresh, `${label}：頁面已重新載入（新的 document）`);
  await waitForFirstProjection(ctx.cdp, ctx.preview.port);
  const want = ['LIVE', ...names];
  const got = await ctx.cdp.poll((w) => JSON.stringify(window.__sc.splitState().names) === JSON.stringify(w), [want], UI_TIMEOUT_MS);
  const now = got ? want : (await splitState(ctx)).names;
  check(!!got, `${label}：重新載入後分頁依序為 ${JSON.stringify(want)}（實際 ${JSON.stringify(now)}）`);
  await sleep(300);
  return at;
}

// 某次載入之後 console 裡提到並排的警告（restoreTabs() 忽略並排組合時寫的那一則）。
const splitWarnings = (ctx, since) => ctx.console.filter((e) => e.at >= since && e.level === 'warning' && /並排/.test(e.text));

// 寫入 data、重新載入、等分頁依序為 names。回傳重新載入的時間。
async function seedAndReload(ctx, data, names, label) {
  need(await writeStored(ctx, data), `${label}：寫入 localStorage（${JSON.stringify({ split: data.split, splitFocus: data.splitFocus, tabs: (data.tabs || []).map(storedName) })}）`);
  return reloadPage(ctx, names, label);
}

// 回滾相容（design D8「為什麼不升到 v3」、Migration Plan）：回滾目標固定為本 change 之前的最後一個發行版 ROLLBACK_REF
// （tag v0.1.2，7f6ce36，不含並排）。取出該版 cockpit/assets/app/files.js 中的 isStoredState() 原文，連同它用到的常數
// （STORAGE_VERSION、LEFT_*），回傳可直接放進 Runtime.evaluate 的程式碼。直接嵌進求值的運算式，不經 eval／new Function，
// 不受頁面 CSP 影響。
// 為什麼用固定 tag，不用 `git merge-base HEAD origin/main`（task 3.6 原本的做法，task 5.2 最終審查 I-1 改掉）：本分支
// squash 併回 main 之後，在 main 上跑時 merge-base 就是 HEAD 自己，取出的是改版後的 isStoredState()，斷言恆真（假綠）；
// 沒有 origin/main 的 clone 也會直接失敗。固定 tag 在合併前後都指向同一個舊版本。另外要求取出的 files.js 不含
// `splitTabs`：回滾目標若誤指到含並排的版本（例如改成 HEAD），這裡就失敗，不會靜默變成自己比自己。
// 之後要拿新的發行版當回滾目標時，改 ROLLBACK_REF。
const ROLLBACK_REF = 'v0.1.2';
function rollbackIsStoredStateSource() {
  const rev = spawnSync('git', ['rev-parse', '--verify', '--quiet', `${ROLLBACK_REF}^{commit}`], { cwd: REPO, encoding: 'utf8' });
  if (rev.status !== 0) return { error: `找不到 ${ROLLBACK_REF}（clone 沒有帶 tag 時先 git fetch --tags）${rev.stderr}` };
  const sha = rev.stdout.trim();
  const show = spawnSync('git', ['show', `${ROLLBACK_REF}:cockpit/assets/app/files.js`], { cwd: REPO, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (show.status !== 0) return { sha, error: `git show ${ROLLBACK_REF}:cockpit/assets/app/files.js 失敗：${show.stderr}` };
  const src = show.stdout.replace(/\r\n/g, '\n');
  if (src.includes('splitTabs')) return { sha, error: `${ROLLBACK_REF} 的 files.js 已含並排（splitTabs），不是回滾目標` };
  const fn = src.match(/\n {2}function isStoredState\(data\) \{\n[\s\S]*?\n {2}\}\n/);
  if (!fn) return { sha, error: '找不到 isStoredState() 的原文' };
  const consts = [];
  for (const name of ['STORAGE_VERSION', 'LEFT_PROJECTS', 'LEFT_FILES', 'LEFT_CHANGES']) {
    const m = src.match(new RegExp(`\\n {2}var ${name} = ([^;\\n]+);`));
    if (!m) return { sha, error: `找不到常數 ${name}` };
    consts.push(`var ${name} = ${m[1]};`);
  }
  return { sha, code: `${consts.join('\n')}\n${fn[0]}` };
}

async function rollbackIsStoredState(ctx, old, data) {
  return ctx.cdp.eval(`(() => {\n${old.code}\nreturn isStoredState(${JSON.stringify(data)});\n})()`);
}

// spec「還原並排」：已打開 README.md、docs/a.md、long.md（spec 的 docs/b.md），其中 long.md 與 README.md 依序並排、焦點欄
// 為 README.md；重新整理 → 三個分頁依原順序還原，long.md、README.md 依序兩欄並排並顯示內容，焦點欄為 README.md。
// 另驗寫入格式：組成並排之前沒有 split／splitFocus 欄位；之後 split 為 [2, 0]、splitFocus 為 0；還原時沒有並排相關的警告。
async function segPersistRestoreSplit() {
  const R = 'README.md';
  const A = 'docs/a.md';
  const B = 'long.md';
  const names = [R, A, B];
  await withCockpit('persist-restore', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of names) await openReady(ctx, f);
    await expectStored(ctx, { names, split: null, current: B }, '組成並排之前');
    await clickTab(ctx, B);
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [B, R], selected: R, shown: [B, R] }, `前置：${B} 為目前分頁時按 ${R} 的並排鈕`);
    await expectStored(ctx, { names, split: [B, R], focus: R, current: R }, '並排後的寫入格式');
    const at = await reloadPage(ctx, names, '重新整理');
    await expectSplit(ctx, { cols: [B, R], selected: R, shown: [B, R] }, '重新整理後');
    await waitFileReady(ctx, B);
    await waitFileReady(ctx, R);
    const warns = splitWarnings(ctx, at);
    check(warns.length === 0, `合法的並排資料還原時沒有並排相關的警告（實際 ${JSON.stringify(warns.map((w) => w.text))}）`);
    noExceptions(ctx);
  });
}

// spec「選定 Live Output 時重新整理」：README.md（a.md）與 docs/a.md（b.md）並排，之後選定 Live Output；重新整理 → 目前分頁
// 為 Live Output（只顯示它，並排組合與欄位標記保留，兩個並排鈕都是可用、aria-pressed="true"）；之後點選 README.md →
// README.md 與 docs/a.md 恢復並排並顯示內容。寫入格式：current 為 null、split 為 [0, 1]、splitFocus 為 1。
async function segPersistLiveOutput() {
  const R = 'README.md';
  const A = 'docs/a.md';
  const names = [R, A];
  await withCockpit('persist-live', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of names) await openReady(ctx, f);
    await clickTab(ctx, R);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [R, A], selected: A, shown: [R, A] }, `前置：${R} 與 ${A} 並排`);
    await clickTab(ctx, 'LIVE');
    await expectSplit(ctx, { cols: [R, A], selected: 'LIVE', shown: ['LIVE'] }, '前置：選定 Live Output');
    await expectStored(ctx, { names, split: [R, A], focus: A, current: 'LIVE' }, '選定 Live Output 後的寫入格式');
    await reloadPage(ctx, names, '重新整理');
    await expectSplit(ctx, { cols: [R, A], selected: 'LIVE', shown: ['LIVE'] }, '重新整理後');
    const u = await splitUi(ctx);
    for (const f of names) {
      check(!!u[f] && u[f].ariaDisabled === null, `重新整理後 ${f} 的並排鈕為可用（沒有 aria-disabled；實際 ${JSON.stringify(u[f] && u[f].ariaDisabled)}）`);
    }
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [R, A], selected: R, shown: [R, A] }, `之後點選 ${R}`);
    await waitFileReady(ctx, R);
    await waitFileReady(ctx, A);
    noExceptions(ctx);
  });
}

// spec「並排資料不合法時忽略」：已打開 README.md、docs/a.md、long.md、note.txt 與 Git Graph，目前分頁為 docs/a.md。以前端寫出
// 的紀錄為底，逐一改成下列並排資料後重新載入，每次都要：其餘分頁依原順序還原、目前分頁為 docs/a.md、沒有並排（沒有欄位
// 標記、只顯示 docs/a.md）、console 有一則並排相關的警告、沒有未捕捉例外。
//   spec 列的五種：只指到一個分頁、指到 4 個分頁、同一個分頁出現兩次、指到 Git Graph、指到還原時被略過的檔案分頁。
//   design D8 另列的：索引越界；另加同一個檔案存了兩筆（兩個不同的索引還原成同一個分頁，也是重複）。
// 「被略過的分頁」：在 README.md 之後插一筆路徑不合法（../bad.md）的檔案紀錄，split 指到它與 docs/a.md（儲存位置 1、2）。
// 若以還原後的位置對照，1、2 會位移成 docs/a.md、long.md 而形成並排；所以另斷言並排組合不是這兩個。最後以同一份 tabs 做
// 正向對照：split 為 [3, 2]（long.md、docs/a.md）→ 依序並排、沒有並排相關的警告（以還原後的位置對照會變成 note.txt、long.md）。
async function segPersistInvalid() {
  const [R, A, L, N] = ['README.md', 'docs/a.md', 'long.md', 'note.txt'];
  const names = [R, A, L, N, 'GRAPH'];
  await withCockpit('persist-invalid', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of [R, A, L, N]) await openReady(ctx, f);
    await openGitGraph(ctx);
    await clickTab(ctx, A);
    const base = await expectStored(ctx, { names, split: null, current: A }, '前置：前端寫出的紀錄');
    const bad = { ...base.tabs[0], path: '../bad.md' };
    const withBad = [base.tabs[0], bad, ...base.tabs.slice(1)];
    const cases = [
      { label: '只指到一個分頁', tabs: base.tabs, split: [0], splitFocus: 0 },
      { label: '指到 4 個分頁', tabs: base.tabs, split: [0, 1, 2, 3], splitFocus: 0 },
      { label: '同一個分頁出現兩次', tabs: base.tabs, split: [0, 0], splitFocus: 0 },
      { label: '同一個檔案存了兩筆、split 指到這兩筆', tabs: [...base.tabs, { ...base.tabs[0] }], split: [0, 5], splitFocus: 0 },
      { label: '指到 Git Graph 分頁', tabs: base.tabs, split: [0, 4], splitFocus: 0 },
      { label: '指到還原時被略過的檔案分頁', tabs: withBad, split: [1, 2], splitFocus: 2, shifted: [A, L] },
      { label: '索引越界', tabs: base.tabs, split: [0, 9], splitFocus: 0 },
    ];
    for (const c of cases) {
      const at = await seedAndReload(ctx, { ...base, tabs: c.tabs, split: c.split, splitFocus: c.splitFocus }, names, c.label);
      const s = await expectSplit(ctx, { cols: [], selected: A, shown: [A] }, `${c.label}：載入後`);
      if (c.shifted) {
        check(JSON.stringify(s.cols) !== JSON.stringify(c.shifted), `${c.label}：並排沒有因位移而指到 ${JSON.stringify(c.shifted)}（實際 ${JSON.stringify(s.cols)}）`);
      }
      await waitFileReady(ctx, A);
      const warns = splitWarnings(ctx, at);
      check(warns.length >= 1, `${c.label}：console 有一則並排相關的警告（載入後的警告：${JSON.stringify(ctx.console.filter((e) => e.at >= at && e.level === 'warning').map((e) => e.text))}）`);
    }
    // 正向對照：被略過的位置之後的索引仍指到正確的分頁。
    const label = '正向對照：略過一筆之後 split [3, 2]';
    const at = await seedAndReload(ctx, { ...base, tabs: withBad, split: [3, 2], splitFocus: 2 }, names, label);
    await expectSplit(ctx, { cols: [L, A], selected: A, shown: [L, A] }, `${label}：依儲存位置還原成 ${L}、${A}`);
    const warns = splitWarnings(ctx, at);
    check(warns.length === 0, `${label}：沒有並排相關的警告（實際 ${JSON.stringify(warns.map((w) => w.text))}）`);
    noExceptions(ctx);
  });
}

// spec「舊格式照常還原」：瀏覽器本機儲存中是只有檔案分頁的舊格式（v1，沒有 kind），記錄了 README.md 與 docs/a.md 兩個檔案
// 分頁 → 兩個檔案分頁依原順序還原並顯示內容，沒有並排。spec 需求另說「沒有並排組合的格式」（v2 沒有 split）也視為沒有
// 並排組合，一併驗。兩種都不該出現並排相關的警告。
async function segPersistOldFormat() {
  const R = 'README.md';
  const A = 'docs/a.md';
  const names = [R, A];
  await withCockpit('persist-old', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of names) await openReady(ctx, f);
    const base = await expectStored(ctx, { names, split: null, current: A }, '前置：前端寫出的紀錄');
    const strip = (e) => {
      const out = { ...e };
      delete out.kind;
      return out;
    };
    const cur = strip(base.current);
    delete cur.rootName; // v1 的 current 沒有 rootName（見 files.js resolveCurrent() 的說明）
    const v1 = { v: 1, tabs: base.tabs.map(strip), current: cur, left: base.left };
    for (const [label, data] of [
      ['v1 舊格式', v1],
      ['v2 沒有 split 的格式', base],
    ]) {
      const at = await seedAndReload(ctx, data, names, label);
      await expectSplit(ctx, { cols: [], selected: A, shown: [A] }, `${label}：載入後`);
      await waitFileReady(ctx, A);
      await clickTab(ctx, R);
      await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, `${label}：點選 ${R} 後`);
      await waitFileReady(ctx, R);
      const warns = splitWarnings(ctx, at);
      check(warns.length === 0, `${label}：沒有並排相關的警告（實際 ${JSON.stringify(warns.map((w) => w.text))}）`);
    }
    noExceptions(ctx);
  });
}

// spec「分頁還原」：儲存的焦點欄不在並排組合中時，改用並排組合的第一欄；目前分頁在並排組合中時，焦點欄一律為目前分頁。
// 焦點欄在目前分頁不是並排成員時看不到，改用 spec「不在並排中時加入已滿的並排組合」觀察：三欄已滿時按第 4 個檔案的並排鈕，
// 它取代焦點欄、位置不變。已打開 README.md、docs/a.md、long.md、note.txt，split 為前三個：
//   1. splitFocus 為 3（note.txt，不在並排組合中）、目前為 Live Output → 按 note.txt 的並排鈕 → 取代第 1 欄。
//   2. 沒有 splitFocus 欄位 → 同上，取代第 1 欄。
//   3. 對照：splitFocus 為 1（docs/a.md）→ 取代第 2 欄（確認這個觀察方式分辨得出焦點欄）。
//   4. splitFocus 為 0（README.md）、目前分頁為 docs/a.md → 還原成三欄並排、docs/a.md 為焦點欄；選定 Live Output 後按 note.txt
//      的並排鈕 → 取代第 2 欄（焦點欄改成了目前分頁，不是儲存的 README.md）。
async function segPersistFocusFallback() {
  const [R, A, L, N] = ['README.md', 'docs/a.md', 'long.md', 'note.txt'];
  const names = [R, A, L, N];
  await withCockpit('persist-focus', {}, async (ctx) => {
    await openTree(ctx);
    for (const f of names) await openReady(ctx, f);
    const base = await expectStored(ctx, { names, split: null, current: N }, '前置：前端寫出的紀錄');
    const cases = [
      { label: 'splitFocus 指到不在並排組合中的 note.txt', focus: 3, current: null, after: [N, A, L] },
      { label: '沒有 splitFocus 欄位', focus: undefined, current: null, after: [N, A, L] },
      { label: '對照：splitFocus 為 docs/a.md', focus: 1, current: null, after: [R, N, L] },
      { label: 'splitFocus 為 README.md、目前分頁為 docs/a.md', focus: 0, current: base.tabs[1], after: [R, N, L] },
    ];
    for (const c of cases) {
      const data = { ...base, split: [0, 1, 2], current: c.current };
      if (c.focus !== undefined) data.splitFocus = c.focus;
      await seedAndReload(ctx, data, names, c.label);
      if (c.current === null) {
        await expectSplit(ctx, { cols: [R, A, L], selected: 'LIVE', shown: ['LIVE'] }, `${c.label}：載入後`);
      } else {
        await expectSplit(ctx, { cols: [R, A, L], selected: A, shown: [R, A, L] }, `${c.label}：載入後（目前分頁在並排組合中，它就是焦點欄）`);
        await clickTab(ctx, 'LIVE');
        await expectSplit(ctx, { cols: [R, A, L], selected: 'LIVE', shown: ['LIVE'] }, `${c.label}：選定 Live Output`);
      }
      await pressSplit(ctx, N);
      await expectSplit(ctx, { cols: c.after, selected: N, shown: c.after }, `${c.label}：按 ${N} 的並排鈕後（取代焦點欄）`);
    }
    noExceptions(ctx);
  });
}

// 寫入格式與回滾相容（design D8、Migration Plan）：
//   1. 寫入時索引在略過之後才計算：打開 README.md、Git Graph、docs/a.md、long.md，把 Git Graph 的 serialize() 換成回傳 null
//      （模擬「不支援還原」的分頁，前端不會存它）。並排 docs/a.md、long.md、README.md，焦點欄 docs/a.md → tabs 為 README.md、
//      docs/a.md、long.md，split 為 [1, 2, 0]，splitFocus 為 1（以分頁列的位置算會是 [2, 3, 0] 與 2）。選定 Live Output 後
//      current 為 null，split／splitFocus 不變。
//   2. 回滾相容：用回滾目標 ROLLBACK_REF（上一個發行版 v0.1.2）的 isStoredState() 檢查這份帶 split 的紀錄，要回傳 true
//      （舊版照常還原分頁，只失去並排）；對照：同一份紀錄改成 v: 3 時回傳 false（確認取出的函式真的會拒絕不認得的版號）。
//   3. 重新載入 → 三個檔案分頁還原，並排組合保留、目前為 Live Output；點選 long.md → 三欄並排。
//   4. 依序移出 README.md 與 docs/a.md，並排解除 → 紀錄裡不再有 split 與 splitFocus。
async function segPersistFormatRollback() {
  const [R, A, L] = ['README.md', 'docs/a.md', 'long.md'];
  const old = rollbackIsStoredStateSource();
  need(!old.error, `取出回滾目標 ${ROLLBACK_REF} 的 isStoredState()（${old.sha || '?'}；${old.error || 'ok'}）`);
  log(`回滾相容：回滾目標 ${ROLLBACK_REF}（${old.sha}），isStoredState() 連同常數共 ${old.code.length} 字`);
  await withCockpit('persist-format', {}, async (ctx) => {
    await openTree(ctx);
    await openReady(ctx, R);
    await openGitGraph(ctx);
    await switchLeftTab(ctx, '檔案');
    await openReady(ctx, A);
    await openReady(ctx, L);
    await expectStored(ctx, { names: [R, 'GRAPH', A, L], split: null, current: L }, '沒有並排組合時');
    need(
      await ctx.cdp.run(() => {
        const g = window.cockpitGit && window.cockpitGit.kinds ? window.cockpitGit.kinds.graph : null;
        if (!g) return false;
        g.serialize = () => null;
        return true;
      }),
      '把 Git Graph kind 的 serialize() 換成回傳 null'
    );
    await clickTab(ctx, A);
    await pressSplit(ctx, L);
    await expectSplit(ctx, { cols: [A, L], selected: L, shown: [A, L] }, `前置：${A} 為目前分頁時按 ${L} 的並排鈕`);
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [A, L, R], selected: R, shown: [A, L, R] }, `前置：再按 ${R} 的並排鈕`);
    await clickTab(ctx, A);
    await expectSplit(ctx, { cols: [A, L, R], selected: A, shown: [A, L, R] }, `前置：點選 ${A}（換焦點欄）`);
    const names = [R, A, L];
    await expectStored(ctx, { names, split: [A, L, R], focus: A, current: A }, '略過不存的分頁之後才算索引');
    await clickTab(ctx, 'LIVE');
    const d = await expectStored(ctx, { names, split: [A, L, R], focus: A, current: 'LIVE' }, '選定 Live Output 後');
    need(Array.isArray(d.split), `回滾相容的前提：紀錄裡帶 split（實際 ${JSON.stringify(d.split)}）`);

    check((await rollbackIsStoredState(ctx, old, d)) === true, `回滾相容：${ROLLBACK_REF}（${old.sha.slice(0, 7)}）的 isStoredState() 對帶 split 的紀錄回傳 true（紀錄 ${JSON.stringify({ v: d.v, split: d.split, splitFocus: d.splitFocus, left: d.left })}）`);
    check((await rollbackIsStoredState(ctx, old, { ...d, v: 3 })) === false, `對照：同一份紀錄改成 v: 3 時，${ROLLBACK_REF} 的 isStoredState() 回傳 false`);

    await reloadPage(ctx, names, '重新載入');
    await expectSplit(ctx, { cols: [A, L, R], selected: 'LIVE', shown: ['LIVE'] }, '重新載入後');
    await clickTab(ctx, L);
    await expectSplit(ctx, { cols: [A, L, R], selected: L, shown: [A, L, R] }, `點選 ${L} 後`);
    await pressSplit(ctx, R);
    await expectSplit(ctx, { cols: [A, L], selected: L, shown: [A, L] }, `移出 ${R}`);
    await pressSplit(ctx, A);
    await expectSplit(ctx, { cols: [], selected: L, shown: [L] }, `移出 ${A}，並排解除`);
    await expectStored(ctx, { names, split: null, current: L }, '並排解除後');
    noExceptions(ctx);
  });
}

// file-split-view task 4.1 的設計審核修正（五條外觀 findings，各自要有實際命中的狀態）。
//   F4（reading-flow）：視窗 1280，README.md、note.txt、long.md 並排，順序是 README.md、note.txt、long.md，但 long.md 的
//     面板在 DOM 裡排在 note.txt 之前。從焦點欄分頁連按 Tab，各欄第一次被走到的順序必須與欄位編號一致；
//     #review 的 reading-flow 並排時為 grid-order、沒有並排時為 normal。
//   F1／F3／F5（視窗 1100，README.md、note.txt、report.pdf 三欄）：
//     F1 每欄的工具列路徑寬度 ≥ 7em（不再被壓成幾個字）。
//     F3 焦點欄除了 --accent 的 1px 框，另有 1px 實線 --accent 外框（outline，不影響 box model）；其他欄沒有；
//        外框不會被祖先元素的 overflow 裁掉（焦點欄在最左、中間與最右各驗一次）。
//     F5 分頁列的欄位徽章：只有目前分頁（焦點欄）是 --accent，其餘是 --text-dim。
//   F2（視窗 800，同一組合）：PDF 工具列的按鈕不逐字直排（white-space: nowrap、單行高度），右緣不超出欄寬。
async function segSplitDesignFixes() {
  const [R, N, L, P] = ['README.md', 'note.txt', 'long.md', 'report.pdf'];
  // F4
  await withCockpit('split-flow', { windowSize: W1280 }, async (ctx) => {
    await need1280(ctx);
    await openTree(ctx);
    // 開檔順序 README.md、long.md、note.txt：面板的 DOM 順序也是這樣。並排組合要排成 README.md、note.txt、long.md，
    // 所以 note.txt 的面板在 DOM 裡排在 long.md 之後，畫面上卻在它左邊。
    for (const f of [R, L, N]) await openReady(ctx, f);
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [], selected: R, shown: [R] }, '並排前');
    const flow0 = await ctx.cdp.run(() => getComputedStyle(document.getElementById('review')).readingFlow);
    check(flow0 === 'normal', `沒有並排時 #review 的 reading-flow 為 normal（實際 ${JSON.stringify(flow0)}）`);
    await pressSplit(ctx, N);
    await expectSplit(ctx, { cols: [R, N], selected: N, shown: [R, N] }, `${R}、${N} 兩欄`);
    await pressSplit(ctx, L);
    await expectSplit(ctx, { cols: [R, N, L], selected: L, shown: [R, N, L] }, `再把 ${L} 加到最右欄`);
    const dom = await reviewChildOrder(ctx);
    log(`#review 子節點順序：${JSON.stringify(dom)}`);
    await clickTab(ctx, N);
    await expectSplit(ctx, { cols: [R, N, L], selected: N, shown: [R, N, L] }, `焦點欄移到中間的 ${N}`);
    const flow1 = await ctx.cdp.run(() => getComputedStyle(document.getElementById('review')).readingFlow);
    check(flow1 === 'grid-order', `並排時 #review 的 reading-flow 為 grid-order（實際 ${JSON.stringify(flow1)}）`);
    const f0 = await ctx.cdp.run(() => window.__sc.focusName());
    need(f0 === N, `點選 ${N} 分頁後鍵盤焦點在它上面（實際 ${JSON.stringify(f0)}）`);
    const trace = [];
    const seen = [];
    for (let i = 0; i < 40 && seen.length < 3; i++) {
      await ctx.cdp.pressKey('Tab', 'Tab', 9, undefined, 0);
      await sleep(60);
      const who = await ctx.cdp.run(() => window.__sc.focusPanelName());
      trace.push(who);
      if (who && who !== 'TABBAR' && !seen.includes(who)) seen.push(who);
    }
    log(`Tab 走過的區塊：${JSON.stringify(trace)}`);
    check(
      JSON.stringify(seen) === JSON.stringify([R, N, L]),
      `連按 Tab，各欄第一次被走到的順序與欄位編號一致 ${JSON.stringify([R, N, L])}（實際 ${JSON.stringify(seen)}；#review 子節點順序 ${JSON.stringify(dom)}）`
    );
    noExceptions(ctx);
  });

  // F1／F3／F5／F2
  await withCockpit('split-look', { windowSize: W1280 }, async (ctx) => {
    // 單欄版面（< 1200 寬）整頁捲動、pane 列的位置不同，所以先在 1280 寬開檔、組好並排，再改寬度（同 segNarrowFocusOnly）。
    await need1280(ctx);
    await openTree(ctx);
    await openReady(ctx, R);
    await openReady(ctx, N);
    await openFile(ctx, P);
    await clickTab(ctx, R);
    await pressSplit(ctx, N);
    await pressSplit(ctx, P);
    await expectSplit(ctx, { cols: [R, N, P], selected: P, shown: [R, N, P] }, `前置：1280 寬 ${R}、${N}、${P} 三欄並排（焦點欄 ${P}）`);
    await setWidth(ctx, 1100);
    await expectSplit(ctx, { cols: [R, N, P], selected: P, shown: [R, N, P] }, `1100 寬三欄並排（焦點欄 ${P}）`);
    const look = (cols, focus) =>
      ctx.cdp.run((cs, fo) => {
        const tok = (n) => {
          const hex = getComputedStyle(document.documentElement).getPropertyValue(n).trim();
          const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
          return m ? `rgb(${parseInt(m[1], 16)}, ${parseInt(m[2], 16)}, ${parseInt(m[3], 16)})` : hex;
        };
        const out = { accent: tok('--accent'), dim: tok('--text-dim'), cols: {} };
        for (const c of cs) {
          const t = window.__sc.fileTab(c);
          const p = window.__sc.panelOf(t);
          const path = p.querySelector('.file-toolbar-path');
          const cp = getComputedStyle(p);
          const after = getComputedStyle(t, '::after');
          // 外框（往外 1px）會不會被任何祖先的 overflow 裁掉。
          const pr = p.getBoundingClientRect();
          const clipped = [];
          for (let a = p.parentElement; a && a !== document.documentElement; a = a.parentElement) {
            const ca = getComputedStyle(a);
            if (ca.overflowX === 'visible' && ca.overflowY === 'visible') continue;
            const ar = a.getBoundingClientRect();
            const o = parseFloat(cp.outlineWidth) || 0;
            if (pr.left - o < ar.left - 0.01 || pr.right + o > ar.right + 0.01 || pr.top - o < ar.top - 0.01 || pr.bottom + o > ar.bottom + 0.01) {
              clipped.push(`${a.tagName.toLowerCase()}#${a.id}.${String(a.className).replace(/ /g, '.')}（overflow ${ca.overflowX}/${ca.overflowY}）`);
            }
          }
          out.cols[c] = {
            pathWidth: path ? path.getBoundingClientRect().width : null,
            pathFont: path ? parseFloat(getComputedStyle(path).fontSize) : null,
            outlineStyle: cp.outlineStyle,
            outlineWidth: cp.outlineWidth,
            outlineColor: cp.outlineColor,
            borderWidth: cp.borderTopWidth,
            borderColor: cp.borderTopColor,
            badgeColor: after.color,
            badgeBorder: after.borderTopColor,
            clipped,
          };
        }
        return out;
      }, cols, focus);
    const checkLook = async (focus, label) => {
      const cols = [R, N, P];
      const v = await look(cols, focus);
      for (const c of cols) {
        const x = v.cols[c];
        const isF = c === focus;
        // F1
        check(x.pathWidth !== null && x.pathWidth >= x.pathFont * 7 - 0.5, `${label}：${c} 的工具列路徑寬度 ≥ 7em（${x.pathWidth}px，7em＝${x.pathFont * 7}px）`);
        // F3
        if (isF) {
          check(x.outlineStyle === 'solid' && x.outlineWidth === '1px' && x.outlineColor === v.accent, `${label}：焦點欄 ${c} 有 1px 實線 --accent 外框（style=${x.outlineStyle}、width=${x.outlineWidth}、color=${x.outlineColor}，--accent=${v.accent}）`);
          check(x.borderWidth === '1px' && x.borderColor === v.accent, `${label}：焦點欄 ${c} 仍是 1px --accent 框線（box model 不變；實際 ${x.borderWidth} ${x.borderColor}）`);
          check(x.clipped.length === 0, `${label}：焦點欄 ${c} 的外框沒有被祖先元素的 overflow 裁掉（${JSON.stringify(x.clipped)}）`);
        } else {
          check(x.outlineStyle === 'none' || x.outlineWidth === '0px', `${label}：非焦點欄 ${c} 沒有外框（style=${x.outlineStyle}、width=${x.outlineWidth}）`);
        }
        // F5
        const want = isF ? v.accent : v.dim;
        check(x.badgeColor === want && x.badgeBorder === want, `${label}：${c} 的欄位徽章${isF ? '（目前分頁）用 --accent' : '用 --text-dim'}（文字 ${x.badgeColor}、框 ${x.badgeBorder}，預期 ${want}）`);
      }
    };
    await checkLook(P, '1100 三欄、焦點欄在最右（report.pdf）');
    await revealTab(ctx, R);
    await clickTab(ctx, R);
    await expectSplit(ctx, { cols: [R, N, P], selected: R, shown: [R, N, P] }, `焦點欄移到最左的 ${R}`);
    await checkLook(R, '1100 三欄、焦點欄在最左（README.md）');
    await revealTab(ctx, N);
    await clickTab(ctx, N);
    await expectSplit(ctx, { cols: [R, N, P], selected: N, shown: [R, N, P] }, `焦點欄移到中間的 ${N}`);
    await checkLook(N, '1100 三欄、焦點欄在中間（note.txt）');

    // F2：800 寬
    await setWidth(ctx, 800);
    await revealTab(ctx, P);
    await clickTab(ctx, P);
    await expectSplit(ctx, { cols: [R, N, P], selected: P, shown: [R, N, P] }, `800 寬三欄（焦點欄 ${P}）`);
    const pdf = await ctx.cdp.run((p) => {
      const panel = window.__sc.panelOf(window.__sc.fileTab(p));
      const pr = panel.getBoundingClientRect();
      const innerRight = pr.left + panel.clientLeft + panel.clientWidth;
      const tools = Array.from(panel.querySelectorAll('.pdf-tool')).map((el) => {
        const r = el.getBoundingClientRect();
        return { text: el.textContent.trim(), ws: getComputedStyle(el).whiteSpace, h: r.height, over: r.right - innerRight };
      });
      const group = panel.querySelector('.pdf-toolbar-group');
      return { tools, wrap: group ? getComputedStyle(group).flexWrap : null, panelWidth: pr.width };
    }, P);
    need(pdf.tools.length >= 4, `PDF 工具列有按鈕（實際 ${JSON.stringify(pdf.tools)}）`);
    log(`800 寬 PDF 工具列：${JSON.stringify(pdf)}`);
    for (const t of pdf.tools) {
      check(t.ws === 'nowrap', `800 寬：PDF 按鈕「${t.text}」white-space 為 nowrap（實際 ${t.ws}）`);
      check(t.h <= 30, `800 寬：PDF 按鈕「${t.text}」單行（高度 ${t.h}px ≤ 30）`);
      check(t.over <= 0.5, `800 寬：PDF 按鈕「${t.text}」右緣不超出欄內緣（超出 ${Math.round(t.over)}px）`);
    }
    check(pdf.wrap === 'wrap', `800 寬：PDF 工具列的按鈕群組 flex-wrap 為 wrap（實際 ${pdf.wrap}）`);
    noExceptions(ctx);
  });
}

// ---------------------------------------------------------------------------
// 段落清單與主程式
// ---------------------------------------------------------------------------

const SEGMENTS = [
  { code: 'self/鷹架', fn: segSelfScaffold, self: true },
  { code: 'self/段落代號', fn: segSelfSegmentArg, self: true },
  { code: 'baseline/沒有並排時只有目前分頁可見', fn: segBaselineOnlyCurrentVisible },
  { code: 'file-review/快速切換時舊回應丟棄', fn: segRapidSwitchDiscardsStale },
  { code: 'file-review/關閉分頁中止進行中的查詢', fn: segCloseAbortsInflight },
  { code: 'file-review/加入並排', fn: segSplitAdd },
  { code: 'file-review/沒有另一個檔案分頁時不並排', fn: segSplitNoPartner },
  { code: 'file-review/替換焦點欄不影響其他欄', fn: segSplitReplaceKeepsOthers },
  { code: 'file-review/從檔案樹開檔替換焦點欄', fn: segSplitReplaceFromTreeAndLink },
  { code: 'file-review/三欄已滿時替換焦點欄', fn: segSplitFullReplace },
  { code: 'file-review/移出焦點欄', fn: segSplitRemoveFocus },
  { code: 'file-review/關閉後只剩一個時解除並排', fn: segSplitCloseDissolves },
  { code: 'file-review/不在並排中時關閉並排組合的成員', fn: segSplitCloseWhileAway },
  { code: 'file-review/不在並排中時加入已滿的並排組合', fn: segSplitAddFullWhileAway },
  { code: 'file-review/切到 Live Output 後整組恢復', fn: segSplitRestoreAfterLive },
  { code: 'file-review/非檔案分頁不能並排', fn: segSplitNonFileTabs },
  { code: 'file-review/並排中的非焦點欄也更新', fn: segSplitNonFocusUpdates },
  { code: 'file-review/並排中各欄輪詢互不影響', fn: segSplitPollingIndependent },
  { code: 'file-review/並排版面等寬', fn: segSplitLayoutEqual },
  { code: 'file-review/三欄並排不撐破頁面', fn: segSplitThreeNoOverflow },
  { code: 'file-review/並排切換不重新載入 iframe', fn: segSplitIframeNotReloaded },
  { code: 'file-review/整頁重畫不影響並排', fn: segSplitRepaint },
  { code: 'file-review/切換 Project 不影響並排', fn: segSplitProjectSwitch },
  { code: 'file-review/Ctrl＋點選加入並排', fn: segSplitCtrlClick },
  { code: 'file-review/鍵盤加入並排', fn: segSplitKeyboard },
  { code: 'file-review/並排鈕停用時 Ctrl＋點選等同一般選定', fn: segSplitDisabledCtrlClick },
  { code: 'file-review/並排鈕的顯示時機與 Tab 順序', fn: segSplitButtonShowAndTab },
  { code: 'file-review/欄位編號標記與說明', fn: segSplitColumnMarks },
  { code: 'file-review/並排鈕文字隨介面語言', fn: segSplitI18n },
  { code: 'file-review/關閉並排的焦點欄時接手分頁捲進視野', fn: segSplitCloseReveals },
  { code: 'file-review/不在並排中移出成員時分頁列不捲動', fn: segSplitAwayNoScroll },
  { code: 'file-review/在欄內點選切換焦點欄', fn: segSplitPointerFocus },
  { code: 'file-review/在非焦點欄點 md 相對連結在那一欄開啟', fn: segSplitLinkInNonFocus },
  { code: 'file-review/在非焦點的 PDF 欄點下一頁照常翻頁', fn: segSplitPdfNextInNonFocus },
  { code: 'file-review/在 html 欄按下滑鼠切換焦點欄', fn: segSplitHtmlColumnPress },
  { code: 'file-review/兩個 html 欄之間直接切換焦點欄', fn: segSplitTwoHtmlColumns },
  { code: 'file-review/窄視窗只顯示焦點欄', fn: segNarrowFocusOnly },
  { code: 'file-review/只查詢可見的檔案分頁', fn: segNarrowQueriesVisibleOnly },
  { code: 'file-review/窄視窗下仍替換焦點欄', fn: segNarrowReplaceFocus },
  { code: 'file-review/窄視窗下按下滑鼠不切換焦點欄', fn: segNarrowPointerFocus },
  { code: 'file-review/還原並排', fn: segPersistRestoreSplit },
  { code: 'file-review/選定 Live Output 時重新整理', fn: segPersistLiveOutput },
  { code: 'file-review/並排資料不合法時忽略', fn: segPersistInvalid },
  { code: 'file-review/舊格式照常還原', fn: segPersistOldFormat },
  { code: 'file-review/焦點欄不在並排組合中時改用第一欄', fn: segPersistFocusFallback },
  { code: 'file-review/寫入格式與回滾相容', fn: segPersistFormatRollback },
  { code: 'file-review/設計審核修正的外觀', fn: segSplitDesignFixes },
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
