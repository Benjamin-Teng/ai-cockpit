// 3.6「未知狀態不破壞畫面」驗證：後端型別是封閉五值送不出 whatever，所以用 harness 頁面直接餵 render.js。
// 只用 Node 22 內建 API。用法（repo 根，先 cargo build -p cockpit --examples）：
//   node docs/research/2026-09-15/whatever-check.js
// 流程：起 ui_preview（自己 spawn，依 PID 清理）→ 取 /api/state → 把 panes[0].agent_status 改成 "whatever"
//       → 產生 harness.html（載入同一份 /app/style.css、/app/render.js，呼叫 window.onState(state)）
//       → headless Chrome --dump-dom → 斷言該列 class 為 status-unknown、文字與 title 為 whatever、
//         沒有 status-whatever 這種未經白名單的 class、其他列正常。
// 清理：整個生命週期在同一個 try/finally 內；暫存目錄刪除失敗算 FAIL。
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');

const REPO = path.resolve(__dirname, '..', '..', '..');
const EXE = path.join(REPO, 'target', 'debug', 'examples', process.platform === 'win32' ? 'ui_preview.exe' : 'ui_preview');
const CHROME = process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
}
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
function killTree(child) {
  if (!child || child.exitCode !== null) return;
  if (process.platform === 'win32') {
    const r = spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
    check(r.status === 0, `taskkill PID ${child.pid} 成功`);
  } else {
    try { child.kill('SIGKILL'); } catch (e) { check(false, `kill ${child.pid} 失敗：${e.message}`); }
  }
}

async function main() {
  let preview = null;
  let dir = null;
  try {
    if (await apiUp()) throw new Error('127.0.0.1:7770 已有服務在跑，請先停掉再執行');
    preview = spawn(EXE, [], { stdio: 'ignore', windowsHide: true });
    preview.on('error', (e) => check(false, `preview spawn error：${e.message}`));
    if (!(await waitApi(true, 20000))) throw new Error('preview did not start');
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-harness-'));
    const state = await (await fetch('http://127.0.0.1:7770/api/state')).json();
    state.runtimes[0].workspaces[0].tabs[0].panes[0].agent_status = 'whatever';
    const html = `<!doctype html><html><head><meta charset="utf-8">
<link rel="stylesheet" href="http://127.0.0.1:7770/app/style.css"></head>
<body><header id="topbar"><span id="channel-status">connecting</span></header><div id="app"></div>
<script src="http://127.0.0.1:7770/app/render.js"></script>
<script>window.onState(${JSON.stringify(state)});</script></body></html>`;
    const harness = path.join(dir, 'harness.html');
    fs.writeFileSync(harness, html, 'utf8');
    const fileUrl = 'file:///' + harness.replace(/\\/g, '/');
    const r = spawnSync(CHROME, [
      '--headless=new', '--disable-gpu', '--no-first-run', '--allow-file-access-from-files',
      `--user-data-dir=${path.join(dir, 'profile')}`, '--virtual-time-budget=3000', '--dump-dom', fileUrl,
    ], { encoding: 'utf8', windowsHide: true, maxBuffer: 16 * 1024 * 1024 });
    const dom = r.stdout || '';
    check(r.status === 0 && dom.length > 0, `headless Chrome dump-dom 成功（${dom.length} bytes）`);
    const rows = dom.match(/<[^>]*class="[^"]*pane-row[^"]*"[^>]*>[\s\S]*?<\/(?:li|tr|div)>/g) || [];
    check(rows.length === 3, `pane 列數為 3（${rows.length}）`);
    const summary = rows.map((row) => {
      const cls = (row.match(/status status-([a-z]+)/) || [])[1];
      const text = (row.match(/class="status[^"]*"[^>]*>([^<]*)</) || [])[1];
      const title = /title="whatever"/.test(row);
      return { cls, text, title };
    });
    console.log('rows:', JSON.stringify(summary));
    const target = summary[0] || {};
    check(target.cls === 'unknown', `whatever 列的色塊 class 為 status-unknown（${target.cls}）`);
    check(target.text === 'whatever', `whatever 列的色塊文字保留原字串（${target.text}）`);
    check(target.title === true, 'whatever 列的色塊 title 屬性為原字串');
    check(!/status-whatever/.test(dom), '沒有出現未經白名單的 status-whatever class');
    check(summary.slice(1).every((s) => ['working', 'blocked', 'done', 'idle', 'unknown'].includes(s.cls)), '其他列的色塊 class 仍在五值白名單內');
  } finally {
    if (preview) {
      killTree(preview);
      check(await waitApi(false, 5000), 'preview 已停止（API 不再回應）');
    }
    if (dir) {
      try { fs.rmSync(dir, { recursive: true, force: true }); } catch (e) { check(false, `清理暫存目錄失敗：${e.message}`); }
    }
  }
  if (failures.length) { console.log(`RESULT: FAIL (${failures.length})`); process.exitCode = 2; }
  else console.log('RESULT: PASS');
}

main().catch((e) => { console.error('FAIL', e); process.exitCode = 1; });
