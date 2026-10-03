// 3.6 通道重連驗證：用 headless Chrome + DevTools protocol 觀察頂列通道狀態與整份重畫。
// 只用 Node 22 內建 API（fetch、WebSocket、child_process），不引 npm 套件。
// 用法（repo 根，先 cargo build -p cockpit --examples）：node docs/research/2026-09-15/reconnect-check.js
// 流程：起 ui_preview → 開頁 → 斷言 connected／兩張卡／version 合法 → 停 preview → 斷言 disconnected
//       → 重啟 → 斷言 connected、version 更新、之後 2.5 s 內 version 再變且 pane 狀態輪替（證明整份重畫）。
// 清理：整個生命週期在同一個 try/finally 內；只終止本腳本自己 spawn 的 preview／Chrome（依 PID），
//       不依映像名稱全域 taskkill；暫存目錄刪除失敗算 FAIL。
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');

const REPO = path.resolve(__dirname, '..', '..', '..');
const EXE = path.join(REPO, 'target', 'debug', 'examples', process.platform === 'win32' ? 'ui_preview.exe' : 'ui_preview');
const CHROME = process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
const PORT = 9333;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);

async function apiUp() {
  try { const r = await fetch('http://127.0.0.1:7770/api/state'); return r.ok; } catch { return false; }
}
async function waitApi(up, timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if ((await apiUp()) === up) return true;
    await sleep(200);
  }
  return false;
}

// spawn 並掛 error handler（非同步 spawn 失敗不會變成未處理例外，而是記進 failures）。
function spawnTracked(cmd, args, label) {
  const child = spawn(cmd, args, { stdio: 'ignore', windowsHide: true });
  child.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
  return child;
}
function killTree(child) {
  if (!child || child.exitCode !== null) return;
  if (process.platform === 'win32') {
    const r = spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
    check(r.status === 0, `taskkill PID ${child.pid} 成功`);
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

async function main() {
  let preview = null;
  let chrome = null;
  let udd = null;
  let ws = null;
  const stopPreview = async () => {
    killTree(preview);
    preview = null;
    check(await waitApi(false, 5000), 'preview 已停止（API 不再回應）');
  };
  try {
    if (await apiUp()) throw new Error('127.0.0.1:7770 已有服務在跑，請先停掉再執行');
    preview = spawnTracked(EXE, [], 'preview');
    if (!(await waitApi(true, 20000))) throw new Error('preview did not start');
    log(`preview up (pid ${preview.pid})`);
    udd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-chrome-'));
    chrome = spawnTracked(CHROME, [
      '--headless=new', '--lang=zh-TW', '--disable-gpu', '--no-first-run',
      `--remote-debugging-port=${PORT}`, '--remote-allow-origins=*',
      `--user-data-dir=${udd}`, '--window-size=1280,900', 'http://127.0.0.1:7770/',
    ], 'chrome');

    let page = null;
    for (let i = 0; i < 100 && !page; i++) {
      try {
        const r = await fetch(`http://127.0.0.1:${PORT}/json/list`);
        page = (await r.json()).find((t) => t.type === 'page' && t.url.startsWith('http://127.0.0.1:7770'));
      } catch {}
      if (!page) await sleep(200);
    }
    if (!page) throw new Error('page target not found');
    ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
    const cdp = new CDP(ws);
    await sleep(1500);
    const status = () => cdp.eval("(document.getElementById('channel-status')||{}).textContent");
    const version = () => cdp.eval("(document.getElementById('version')||{}).textContent");
    const cards = () => cdp.eval("document.querySelectorAll('.runtime-card').length");
    const paneStatus = () => cdp.eval("(document.querySelector('.pane-row .status')||{}).textContent");
    const validVersion = (v) => typeof v === 'string' && /^v\d+$/.test(v);

    const s0 = await status(), v0 = await version(), c0 = await cards();
    log(`T0: status=${s0} version=${v0} cards=${c0}`);
    check(s0 === 'connected', 'T0 通道狀態為 connected');
    check(validVersion(v0), `T0 version 合法（${v0}）`);
    check(c0 === 2, `T0 兩張 runtime 卡（${c0}）`);

    await stopPreview();
    await sleep(1500);
    const s1 = await status(), v1 = await version();
    log(`after stop: status=${s1} version=${v1}`);
    check(s1 === 'disconnected', '停掉 preview 後通道狀態為 disconnected');

    preview = spawnTracked(EXE, [], 'preview');
    if (!(await waitApi(true, 20000))) throw new Error('preview did not restart');
    let s2 = null, v2 = null;
    for (let i = 0; i < 40; i++) {
      s2 = await status(); v2 = await version();
      if (s2 === 'connected' && validVersion(v2) && v2 !== v1) break;
      await sleep(500);
    }
    const c2 = await cards(), p2 = await paneStatus();
    log(`after restart: status=${s2} version=${v2} (before=${v1}) cards=${c2} pane0=${p2}`);
    check(s2 === 'connected', '重啟後通道狀態回到 connected');
    check(validVersion(v2) && v2 !== v1, `重啟後收到新的整份投影（version ${v1} → ${v2}）`);
    check(c2 === 2, `重啟後仍有兩張 runtime 卡（${c2}）`);

    await sleep(2500);
    const v3 = await version(), p3 = await paneStatus();
    log(`+2.5s: version=${v3} pane0=${p3}`);
    check(validVersion(v3) && v3 !== v2, `重連後持續收到推送（version ${v2} → ${v3}）`);
    check(typeof p2 === 'string' && typeof p3 === 'string' && p2 !== p3, `pane 狀態色塊隨推送重畫（${p2} → ${p3}）`);
  } finally {
    try { if (ws) ws.close(); } catch {}
    killTree(chrome);
    if (preview) await stopPreview();
    await sleep(500);
    if (udd) {
      try { fs.rmSync(udd, { recursive: true, force: true }); } catch (e) { check(false, `清理暫存目錄失敗：${e.message}`); }
    }
  }
  if (failures.length) { console.log(`RESULT: FAIL (${failures.length})`); process.exitCode = 2; }
  else console.log('RESULT: PASS');
}

main().catch((e) => { console.error('FAIL', e); process.exitCode = 1; });
