// Task 5.1 驗收：擴充 docs/research/2026-09-15/reconnect-check.js 的做法，直接觀察
// cockpit/assets/app/channel.js 的行為（spec cockpit-dashboard「連上即斷不歸零退避」
// 「壞訊息不中斷」）。fix round 1（Codex finding 1）：退避重設要在「收到第一則訊息」時發生，
// 不限訊息解析成功——所以除了「完全沒收到訊息就不歸零」，還要正面驗證「退避已經升高之後，
// 收到一則訊息（不論合法或不合法 JSON）就會歸零」，否則測不出「重設點放在 JSON.parse 之前
// 還是之後」這個差異：
//   1) 無訊息不重設——一個「接受 WebSocket 後立即關閉、不送任何訊息」的測試 server，逐次
//      記錄重連（HTTP Upgrade）到達的時間，驗證間隔依 1、2、4、8、8 秒遞增、不回到 1 秒。
//   2) 壞訊息後正常訊息會重畫——另一個維持連線的測試 server，送一則 `{not json`，等一下再
//      送一份合法 JSON，驗證第一則被略過並 console.warn、第二則正常送進 window.onState、
//      期間通道狀態不曾變成 disconnected。
//   3) 退避升到 ≥4 秒後收到訊息應重設為約 1 秒——測試 server 先讓前三次連線都立即關閉、不送
//      訊息（退避依序升到 1、2、4 秒，backoffIndex 來到「下一次是 8 秒」的狀態），第四次連線
//      送一則訊息後關閉，驗證下一次（第五次）重連間隔回到約 1 秒而不是 8 秒。跑兩輪：訊息分別
//      是合法 JSON 與 `{not json`，兩輪都要重設，才能證明重設點確實在 JSON.parse 之前。
// 情境 1 與情境 2／3 的測試 server 行為互斥（不可能「立刻關閉」又「維持連線送訊息」），所以
// 用各自獨立的測試 server 設定、各自獨立的 headless Chrome session 觀察，不共用一個 server。
//
// 只用 Node 22 內建 API（http、net、crypto、child_process、WebSocket 只在讀 CDP 時當「客戶端」
// 用）；沒有內建 WebSocket *server*，所以這裡用 http 模組的 'upgrade' 事件手寫最小 WS 握手
// （Sec-WebSocket-Accept）與 server→client 文字訊框（server 端不需要 mask，見 RFC 6455
// §5.1）。不引入任何 npm 套件。
//
// 頁面本身也是這支腳本用 http 模組現場生成，不是 cockpit 的完整 index.html／render.js——
// task 5.1 的範圍只有 channel.js，直接綁 window.onChannel／window.onState／console.warn
// 記錄到陣列，比套完整 app 畫面更貼近「只測 channel.js」且不會被 render.js 的行為干擾。
// channel.js 本身仍是讀 cockpit/assets/app/channel.js 的實際內嵌檔內容（不是複製一份），
// 確保測的是正式檔案。
//
// 用法（repo 根）：node docs/research/2026-09-16/channel-backoff-check.js
// 清理：兩個情境各自在自己的 try/finally 內；只終止本腳本自己 spawn 的 Chrome（依 PID），
//       不依映像名稱全域 taskkill；暫存目錄刪除失敗算 FAIL。
const http = require('node:http');
const crypto = require('node:crypto');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');

const REPO = path.resolve(__dirname, '..', '..', '..');
const CHANNEL_JS_PATH = path.join(REPO, 'cockpit', 'assets', 'app', 'channel.js');
const CHROME = process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
const WS_GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);

function spawnTracked(cmd, args, label) {
  const child = spawn(cmd, args, { stdio: 'ignore', windowsHide: true });
  child.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
  return child;
}
function pidStillRunning(pid) {
  const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
  return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
}

function killTree(child) {
  if (!child || child.exitCode !== null) return;
  if (process.platform === 'win32') {
    spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
    // Chrome 是多行程架構，/T 殺行程樹時常有一兩支子行程（GPU／utility）剛好搶先退出或受
    // 保護，taskkill 對這些回報非 0（甚至 255）——不代表清理失敗。用 tasklist 直接查主行程
    // 的 PID 還在不在，才是「有沒有清乾淨」的真正判準；查不到＝主行程確實沒了。
    check(!pidStillRunning(child.pid), `chrome PID ${child.pid} 已終止（tasklist 查無此 PID）`);
  } else {
    try { child.kill('SIGKILL'); } catch (e) { check(false, `kill ${child.pid} 失敗：${e.message}`); }
  }
}

class CDP {
  constructor(ws) {
    this.ws = ws; this.id = 0; this.pending = new Map();
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) { this.pending.get(m.id)(m); this.pending.delete(m.id); }
    };
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((res) => { this.pending.set(id, res); this.ws.send(JSON.stringify({ id, method, params })); });
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
    } catch {}
    if (!page) await sleep(200);
  }
  if (!page) throw new Error('page target not found');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  return { ws, cdp: new CDP(ws) };
}

async function launchHeadlessChrome(cdpPort, url, label) {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-chrome-'));
  const chrome = spawnTracked(CHROME, [
    '--headless=new', '--lang=zh-TW', '--disable-gpu', '--no-first-run',
    `--remote-debugging-port=${cdpPort}`, '--remote-allow-origins=*',
    `--user-data-dir=${udd}`, '--window-size=1024,768', url,
  ], label);
  return { chrome, udd };
}

// 記錄頁面：綁 window.onChannel／window.onState／console.warn 到陣列供 CDP eval 讀取，
// 不依賴 render.js／完整 app 畫面（task 5.1 只測 channel.js 本身的行為）。
const HARNESS_HTML = `<!doctype html>
<html><head><meta charset="utf-8"><title>channel.js harness</title></head>
<body>
<script>
  window.__events = [];
  window.__warnings = [];
  window.__states = [];
  var origWarn = console.warn.bind(console);
  console.warn = function () {
    window.__warnings.push(Array.prototype.slice.call(arguments).map(String).join(' '));
    origWarn.apply(console, arguments);
  };
  window.onChannel = function (status) { window.__events.push({ t: Date.now(), status: status }); };
  window.onState = function (state) { window.__states.push({ t: Date.now(), state: state }); };
</script>
<script src="/app/channel.js"></script>
</body></html>`;

function acceptKeyFor(clientKey) {
  return crypto.createHash('sha1').update(clientKey + WS_GUID).digest('base64');
}

function completeHandshake(req, socket) {
  const key = req.headers['sec-websocket-key'];
  const accept = acceptKeyFor(key);
  socket.write([
    'HTTP/1.1 101 Switching Protocols',
    'Upgrade: websocket',
    'Connection: Upgrade',
    `Sec-WebSocket-Accept: ${accept}`,
    '', '',
  ].join('\r\n'));
}

// server→client 文字訊框不需要 mask（RFC 6455 §5.1：只有 client→server 一定要 mask）。
// 這裡的測試訊息都很短，固定用 7-bit 長度欄位即可，不處理擴充長度。
function encodeTextFrame(str) {
  const payload = Buffer.from(str, 'utf8');
  if (payload.length >= 126) throw new Error('測試訊息長度應小於 126 bytes，簡化訊框編碼');
  return Buffer.concat([Buffer.from([0x81, payload.length]), payload]);
}

function harnessServer(onUpgrade) {
  const server = http.createServer((req, res) => {
    if (req.url === '/') {
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
      res.end(HARNESS_HTML);
      return;
    }
    if (req.url === '/app/channel.js') {
      res.writeHead(200, { 'Content-Type': 'text/javascript; charset=utf-8' });
      res.end(fs.readFileSync(CHANNEL_JS_PATH, 'utf8'));
      return;
    }
    res.writeHead(404); res.end();
  });
  server.on('upgrade', onUpgrade);
  return server;
}

// ---------------------------------------------------------------------------
// 情境一：接受後立即關閉，觀察重連間隔 1、2、4、8、8 秒。
// ---------------------------------------------------------------------------
async function scenarioBackoff() {
  log('=== 情境一：連上即斷，觀察退避間隔 ===');
  const PORT = 18781;
  const CDP_PORT = 18782;
  const attemptTimes = [];
  const server = harnessServer((req, socket) => {
    if (req.url !== '/ws') { socket.destroy(); return; }
    attemptTimes.push(Date.now());
    completeHandshake(req, socket);
    socket.end(); // 接受後立即關閉、不送任何訊息。
  });
  let chrome = null, udd = null, ws = null;
  try {
    await new Promise((res) => server.listen(PORT, '127.0.0.1', res));
    log(`harness server up on :${PORT}`);
    ({ chrome, udd } = await launchHeadlessChrome(CDP_PORT, `http://127.0.0.1:${PORT}/`, 'chrome-backoff'));

    const t0 = Date.now();
    // 1+2+4+8+8 = 23s 的重連序列走完（第 6 次連線嘗試），加上握手／排程餘裕的上限。
    const deadline = t0 + 35000;
    while (attemptTimes.length < 6 && Date.now() < deadline) {
      await sleep(200);
    }
    check(attemptTimes.length >= 6, `觀察到至少 6 次連線嘗試（實際 ${attemptTimes.length}）`);

    const deltasSec = [];
    for (let i = 1; i < Math.min(attemptTimes.length, 6); i++) {
      deltasSec.push((attemptTimes[i] - attemptTimes[i - 1]) / 1000);
    }
    log(`連線嘗試間隔（秒）：${deltasSec.map((d) => d.toFixed(2)).join(', ')}`);

    const expected = [1, 2, 4, 8, 8];
    const tolerance = 0.4; // headless Chrome + Windows 排程器抖動的容許誤差。
    for (let i = 0; i < expected.length; i++) {
      const d = deltasSec[i];
      check(
        typeof d === 'number' && Math.abs(d - expected[i]) <= tolerance,
        `間隔 #${i + 1} 應約為 ${expected[i]}s（實際 ${d === undefined ? 'N/A' : d.toFixed(2)}s，容許 ±${tolerance}s）`
      );
    }
  } finally {
    try { if (ws) ws.close(); } catch {}
    killTree(chrome);
    await sleep(500); // 讓已終止行程釋放暫存目錄檔案控制代碼，避免刪除時 EBUSY。
    await new Promise((res) => server.close(res));
    if (udd) {
      try { fs.rmSync(udd, { recursive: true, force: true }); } catch (e) { check(false, `清理暫存目錄失敗：${e.message}`); }
    }
  }
}

// ---------------------------------------------------------------------------
// 情境二：壞訊息不中斷——送 `{not json` 後再送一份合法 JSON，確認略過＋console.warn＋
// 下一份正常送進 onState，期間通道狀態不曾變成 disconnected。
// ---------------------------------------------------------------------------
async function scenarioBadMessage() {
  log('=== 情境二：壞訊息不中斷 ===');
  const PORT = 18783;
  const CDP_PORT = 18784;
  const GOOD_STATE = { version: 9, marker: 'task-5.1-good-state' };
  let clientSocket = null;
  const server = harnessServer((req, socket) => {
    if (req.url !== '/ws') { socket.destroy(); return; }
    completeHandshake(req, socket);
    clientSocket = socket;
    // 連線維持開啟：先送一則壞訊息，等一下再送一份合法投影。
    setTimeout(() => {
      if (!socket.destroyed) socket.write(encodeTextFrame('{not json'));
    }, 500);
    setTimeout(() => {
      if (!socket.destroyed) socket.write(encodeTextFrame(JSON.stringify(GOOD_STATE)));
    }, 1200);
  });
  let chrome = null, udd = null, ws = null;
  try {
    await new Promise((res) => server.listen(PORT, '127.0.0.1', res));
    log(`harness server up on :${PORT}`);
    ({ chrome, udd } = await launchHeadlessChrome(CDP_PORT, `http://127.0.0.1:${PORT}/`, 'chrome-badmsg'));
    ({ ws } = await attachCdp(CDP_PORT, `http://127.0.0.1:${PORT}/`));
    const cdp = new CDP(ws);

    await sleep(2000); // 涵蓋 500ms 壞訊息 + 1200ms 合法訊息的排程。

    const events = await cdp.eval('window.__events');
    const warnings = await cdp.eval('window.__warnings');
    const states = await cdp.eval('window.__states');
    log(`events=${JSON.stringify(events)}`);
    log(`warnings=${JSON.stringify(warnings)}`);
    log(`states=${JSON.stringify(states)}`);

    check(Array.isArray(events) && events.length >= 1 && events.every((e) => e.status === 'connected'),
      `通道狀態全程只有 connected、未曾 disconnected（實際 ${JSON.stringify(events)}）`);
    check(Array.isArray(warnings) && warnings.length >= 1,
      `壞訊息應該觸發至少一次 console.warn（實際 ${warnings ? warnings.length : 'N/A'} 次）`);
    check(
      Array.isArray(states) && states.length === 1 &&
        states[0].state && states[0].state.marker === GOOD_STATE.marker,
      `壞訊息之後只有合法投影送進 onState、且內容正確（實際 ${JSON.stringify(states)}）`
    );
  } finally {
    try { if (clientSocket) clientSocket.destroy(); } catch {}
    try { if (ws) ws.close(); } catch {}
    killTree(chrome);
    await sleep(500); // 讓已終止行程釋放暫存目錄檔案控制代碼，避免刪除時 EBUSY。
    await new Promise((res) => server.close(res));
    if (udd) {
      try { fs.rmSync(udd, { recursive: true, force: true }); } catch (e) { check(false, `清理暫存目錄失敗：${e.message}`); }
    }
  }
}

// ---------------------------------------------------------------------------
// 情境三：退避升到 ≥4 秒後收到訊息（合法或不合法 JSON 皆測一次）應重設為約 1 秒。
// 測試 server 對前三次連線都「立即關閉、不送訊息」（退避依序升到 1、2、4 秒），第四次連線
// 送一則訊息（依 kind 決定合法或不合法）後關閉；驗證第三→第四次間隔約 4 秒（退避確實升高），
// 第四→第五次間隔約 1 秒（收到訊息後歸零，不論訊息是否解析成功）。
// ---------------------------------------------------------------------------
async function scenarioResetOnMessage(kind, port, cdpPort) {
  const label = kind === 'valid' ? '合法 JSON' : '壞 JSON';
  log(`=== 情境三（${label}）：退避升到 ≥4 秒後收到訊息應重設為約 1 秒 ===`);
  const payload = kind === 'valid' ? JSON.stringify({ version: 1, marker: 'reset-check' }) : '{not json';
  // 前三次連線（index 0、1、2）立即關閉、不送訊息；第四次（index 3）送一則訊息後關閉；
  // 之後（index >= 4）維持立即關閉，只是為了量出第 5 次連線抵達的時間，不需要再送訊息。
  const behaviors = [null, null, null, payload];
  const attemptTimes = [];
  const server = harnessServer((req, socket) => {
    if (req.url !== '/ws') { socket.destroy(); return; }
    const idx = attemptTimes.length;
    attemptTimes.push(Date.now());
    completeHandshake(req, socket);
    const behavior = idx < behaviors.length ? behaviors[idx] : null;
    if (behavior === null) {
      socket.end(); // 接受後立即關閉、不送任何訊息。
    } else {
      socket.end(encodeTextFrame(behavior)); // 先寫訊框再結束連線。
    }
  });
  let chrome = null, udd = null, ws = null;
  try {
    await new Promise((res) => server.listen(port, '127.0.0.1', res));
    log(`harness server up on :${port}`);
    ({ chrome, udd } = await launchHeadlessChrome(cdpPort, `http://127.0.0.1:${port}/`, `chrome-reset-${kind}`));

    const t0 = Date.now();
    // 需要走到第 5 次連線（index 4）才能量出「收到訊息後那一次重連」的間隔：
    // 1s + 2s + 4s + 1s（重設後）= 8s，加上握手／排程餘裕的上限。
    const deadline = t0 + 20000;
    while (attemptTimes.length < 5 && Date.now() < deadline) {
      await sleep(200);
    }
    check(attemptTimes.length >= 5, `觀察到至少 5 次連線嘗試（實際 ${attemptTimes.length}）`);

    const deltaSec = (i) => (attemptTimes[i] - attemptTimes[i - 1]) / 1000;
    const climbDelta = attemptTimes.length > 3 ? deltaSec(3) : undefined;
    const resetDelta = attemptTimes.length > 4 ? deltaSec(4) : undefined;
    log(
      `間隔（秒）：#1=${attemptTimes.length > 1 ? deltaSec(1).toFixed(2) : 'N/A'} ` +
        `#2=${attemptTimes.length > 2 ? deltaSec(2).toFixed(2) : 'N/A'} ` +
        `#3（升到 4s）=${climbDelta === undefined ? 'N/A' : climbDelta.toFixed(2)} ` +
        `#4（收訊息後應回 1s）=${resetDelta === undefined ? 'N/A' : resetDelta.toFixed(2)}`
    );

    const tolerance = 0.4;
    check(
      typeof climbDelta === 'number' && Math.abs(climbDelta - 4) <= tolerance,
      `退避在收到訊息前應該已經升到約 4s（實際 ${climbDelta === undefined ? 'N/A' : climbDelta.toFixed(2)}s）`
    );
    check(
      typeof resetDelta === 'number' && Math.abs(resetDelta - 1) <= tolerance,
      `收到${label} 訊息後，下一次重連間隔應該回到約 1s，不是 8s（實際 ${resetDelta === undefined ? 'N/A' : resetDelta.toFixed(2)}s）`
    );
  } finally {
    try { if (ws) ws.close(); } catch {}
    killTree(chrome);
    await sleep(500); // 讓已終止行程釋放暫存目錄檔案控制代碼，避免刪除時 EBUSY。
    await new Promise((res) => server.close(res));
    if (udd) {
      try { fs.rmSync(udd, { recursive: true, force: true }); } catch (e) { check(false, `清理暫存目錄失敗：${e.message}`); }
    }
  }
}

async function main() {
  if (!fs.existsSync(CHANNEL_JS_PATH)) throw new Error(`找不到 ${CHANNEL_JS_PATH}`);
  await scenarioBackoff();
  await scenarioBadMessage();
  await scenarioResetOnMessage('valid', 18785, 18786);
  await scenarioResetOnMessage('invalid', 18787, 18788);
  if (failures.length) { console.log(`RESULT: FAIL (${failures.length})`); process.exitCode = 2; }
  else console.log('RESULT: PASS');
}

main().catch((e) => { console.error('FAIL', e); process.exitCode = 1; });
