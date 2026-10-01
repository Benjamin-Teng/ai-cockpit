// progress-model task 4.2／4.3 設計審核截圖：Project p（含 undeclared 列與可退回的節點），
// viewport 寬 1536、1100、700 各一張，存成同目錄 progress-<寬>.png。
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-10-01/progress-screenshots.js
// 只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID）。
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const CHROME = process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
const PORT = 7770;
const CDP_PORT = 18890;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const kill = (c) => c && c.exitCode === null && spawnSync('taskkill', ['/PID', String(c.pid), '/T', '/F']);

async function main() {
  const server = spawn(EXE, [], {
    stdio: 'ignore',
    windowsHide: true,
    // 推送週期拉長，截圖時畫面穩定（pane 狀態輪替不會在截圖當下重畫）。
    env: { ...process.env, COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${PORT}`, COCKPIT_PREVIEW_PUSH_MS: '60000' },
  });
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-chrome-shot-'));
  const url = `http://127.0.0.1:${PORT}/`;
  const chrome = spawn(
    CHROME,
    ['--headless=new', '--disable-gpu', '--no-first-run', `--remote-debugging-port=${CDP_PORT}`,
      '--remote-allow-origins=*', `--user-data-dir=${udd}`, '--window-size=1536,1000', url],
    { stdio: 'ignore', windowsHide: true }
  );
  try {
    let page = null;
    for (let i = 0; i < 100 && !page; i++) {
      try {
        page = (await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json()).find(
          (t) => t.type === 'page' && t.url.startsWith(url)
        );
      } catch {
        // 還沒起來。
      }
      if (!page) await sleep(200);
    }
    if (!page) throw new Error('page target not found');
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((res, rej) => ((ws.onopen = res), (ws.onerror = rej)));
    let id = 0;
    const pending = new Map();
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && pending.has(m.id)) (pending.get(m.id)(m), pending.delete(m.id));
    };
    const send = (method, params = {}) =>
      new Promise((res) => {
        const i = ++id;
        pending.set(i, res);
        ws.send(JSON.stringify({ id: i, method, params }));
      });
    const ev = async (expression) =>
      (await send('Runtime.evaluate', { expression, returnByValue: true })).result.result.value;

    for (let i = 0; i < 50 && !(await ev("!!document.querySelector('.task-node')")); i++) await sleep(200);
    await ev("document.querySelector('[data-action=\"select-project\"][data-project=\"p\"]').dispatchEvent(new PointerEvent('pointerdown', {bubbles: true, button: 0})); true");
    for (let i = 0; i < 50 && !(await ev("!!document.querySelector('.ff-undeclared')")); i++) await sleep(100);

    for (const [w, h] of [[1536, 1300], [1100, 1400], [700, 1400]]) {
      await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
      await sleep(400);
      const shot = await send('Page.captureScreenshot', { format: 'png' });
      const file = path.join(__dirname, `progress-${w}.png`);
      fs.writeFileSync(file, Buffer.from(shot.result.data, 'base64'));
      console.log(`wrote ${file}`);
    }
    ws.close();
  } finally {
    kill(chrome);
    kill(server);
    await sleep(500);
    fs.rmSync(udd, { recursive: true, force: true });
  }
}

main().catch((e) => {
  console.error('FAIL', e);
  process.exitCode = 1;
});
