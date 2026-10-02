// desktop-launch-notify task 2.5 驗收：cockpit --exit-when-idle 的閒置自動結束（spec
// desktop-launch「閒置自動結束」；design D6、D10）。真的啟動 target/debug/cockpit.exe，用 Node
// 內建的 WebSocket 與 fetch 當客戶端，驗證四個情境（各自獨立的程序、埠、暫存目錄）：
//   a) 連上 /ws 再關閉 → 約 10 秒後開始關閉，結束碼 0
//   b) 連上再關閉，3 秒後重連 → 持續執行（關閉後 15 秒時仍在）
//   c) 連上再關閉，8 秒時 GET /api/state，12 秒時重連 → 持續執行（沒有 GET 的話 10 秒就會結束）
//   d) 不帶 --exit-when-idle：連上再關閉，等 20 秒 → 持續執行
// 「從未連線 60 秒」由 cockpit/tests/idle_exit.rs 的單元測試涵蓋（時鐘可撥快），這裡不重跑。
//
// 臨時設定在 repo 之外（os.tmpdir()）：監聽埠為向系統要來的空閒 loopback 埠，runtime 指向
// 不存在的 named pipe，所以完全不會碰 HERDR；cockpit 的工作目錄也在暫存目錄（狀態檔不落在 repo）。
//
// 「開始關閉」的時間點取自 cockpit 日誌（stdout／stderr 導到暫存檔）中「開始正常關閉」那一行
// 行首的 tracing 時間戳（UTC，微秒精度；與本腳本同一台機器、同一個時鐘）；結束時間點取自 exit
// 事件。關閉流程很快，行出現與程序結束只差毫秒，所以不能靠輪詢檔案抓。日誌文字或時間戳格式
// 改了，這支腳本要跟著改（app.rs 的 shutdown_signal 附近）。
//
// 用法（repo 根；先 cargo build -p cockpit）：
//   node docs/research/2026-10-02/idle-exit-check.js
// 約 70 秒，不可與其他驗收腳本並行。每個情境的程序都依 PID 終止；暫存目錄刪除失敗算 FAIL。
const { spawn, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const net = require('node:net');
const os = require('node:os');
const path = require('node:path');

const REPO = path.resolve(__dirname, '..', '..', '..');
const EXE = path.join(REPO, 'target', 'debug', process.platform === 'win32' ? 'cockpit.exe' : 'cockpit');

const sleep = (ms) => new Promise((r) => setTimeout(r, Math.max(0, ms)));
const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'PASS' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const secs = (ms) => (ms / 1000).toFixed(1);

function freePort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.once('error', reject);
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

function pidAlive(pid) {
  if (process.platform === 'win32') {
    const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
    return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

// 一個受測的 cockpit 程序：自己的暫存目錄、埠、日誌。
class Cockpit {
  constructor(label, exitWhenIdle) {
    this.label = label;
    this.exitWhenIdle = exitWhenIdle;
    this.child = null;
    this.exit = null; // { code, signal, at }
  }

  async start() {
    this.dir = fs.mkdtempSync(path.join(os.tmpdir(), `cockpit-idle-${this.label}-`));
    this.port = await freePort();
    const cfg = path.join(this.dir, 'cockpit.toml');
    // TOML 的單引號字串是字面值，Windows 路徑的反斜線不必跳脫。
    fs.writeFileSync(
      cfg,
      `[server]\nlisten = "127.0.0.1:${this.port}"\n\n[[runtime]]\nid = "fake"\nkind = "herdr"\n` +
        `socket = '${path.join(this.dir, 'nonexistent', 'herdr.sock')}'\n`
    );
    this.logPath = path.join(this.dir, 'cockpit.out.log');
    const out = fs.openSync(this.logPath, 'w');
    const args = ['--config', cfg];
    if (this.exitWhenIdle) args.push('--exit-when-idle');
    this.child = spawn(EXE, args, { stdio: ['ignore', out, out], cwd: this.dir, windowsHide: true });
    fs.closeSync(out);
    this.pid = this.child.pid;
    this.child.on('error', (e) => check(false, `${this.label}：spawn 失敗：${e.message}`));
    this.child.on('exit', (code, signal) => {
      this.exit = { code, signal, at: Date.now() };
    });
    // 等監聽埠起來（只做 TCP 連線，不送 HTTP，免得動到閒置計時）。
    for (let i = 0; i < 100; i++) {
      if (this.exit) throw new Error(`${this.label}：cockpit 啟動就結束（碼 ${this.exit.code}）：\n${this.tail()}`);
      if (await this.portOpen()) return;
      await sleep(100);
    }
    throw new Error(`${this.label}：cockpit 10 秒內沒開始監聽 ${this.port}`);
  }

  // 日誌「開始正常關閉」那一行的時間戳（毫秒 epoch）；沒有這一行回 null。
  get shutdownAt() {
    let text;
    try {
      text = fs.readFileSync(this.logPath, 'utf8');
    } catch {
      return null;
    }
    const line = text.split('\n').find((l) => l.includes('開始正常關閉'));
    if (!line) return null;
    const t = Date.parse(line.split(/\s+/)[0]);
    return Number.isNaN(t) ? null : t;
  }

  portOpen() {
    return new Promise((resolve) => {
      const s = net.connect(this.port, '127.0.0.1');
      s.once('connect', () => {
        s.destroy();
        resolve(true);
      });
      s.once('error', () => resolve(false));
    });
  }

  tail() {
    try {
      return fs.readFileSync(this.logPath, 'utf8').split('\n').slice(-10).join('\n');
    } catch {
      return '(無日誌)';
    }
  }

  // 連上 /ws，等到 open；回傳 WebSocket。
  async connect() {
    const ws = new WebSocket(`ws://127.0.0.1:${this.port}/ws`);
    await new Promise((resolve, reject) => {
      ws.onopen = resolve;
      ws.onerror = () => reject(new Error(`${this.label}：/ws 連線失敗`));
    });
    return ws;
  }

  // 關閉並等到客戶端的 close 事件；回傳此刻（「最近一次降為 0」的近似點）。
  async closeWs(ws) {
    await new Promise((resolve) => {
      ws.onclose = resolve;
      ws.close();
    });
    return Date.now();
  }

  get running() {
    return this.exit === null;
  }

  async stop() {
    const pid = this.pid;
    if (pid && this.running) {
      if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], { encoding: 'utf8' });
      else {
        try {
          process.kill(pid, 'SIGKILL');
        } catch {
          /* 已結束 */
        }
      }
      for (let i = 0; i < 25 && pidAlive(pid); i++) await sleep(200);
    }
    if (pid) check(!pidAlive(pid), `${this.label}：PID ${pid} 已不存在`);
    try {
      fs.rmSync(this.dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
      check(!fs.existsSync(this.dir), `${this.label}：暫存目錄已刪除`);
    } catch (e) {
      check(false, `${this.label}：暫存目錄刪除失敗：${e.message}`);
    }
  }
}

// 等到 predicate 為真或逾時；回傳是否成立。
async function waitUntil(pred, timeoutMs, stepMs = 100) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    if (pred()) return true;
    await sleep(stepMs);
  }
  return pred();
}

async function runCase(label, exitWhenIdle, body) {
  log(`=== 情境 ${label} ===`);
  const c = new Cockpit(label, exitWhenIdle);
  try {
    await c.start();
    log(`${label}：cockpit PID ${c.pid}，埠 ${c.port}`);
    await body(c);
  } catch (e) {
    check(false, `${label}：例外：${e.message}`);
  } finally {
    await c.stop();
  }
}

async function main() {
  if (!fs.existsSync(EXE)) {
    console.error(`找不到 ${EXE}；請先在 repo 根執行 cargo build -p cockpit`);
    process.exit(2);
  }

  // a) 最後一個畫面關閉後約 10 秒開始關閉，結束碼 0。
  await runCase('a-閒置結束', true, async (c) => {
    const ws = await c.connect();
    await sleep(300);
    const t0 = await c.closeWs(ws);
    log('a：連線已關閉，等待後端自行結束（上限 10 + 11 + 緩衝秒）');
    // 9 秒前不得開始關閉也不得結束。
    await sleep(9000 - (Date.now() - t0));
    check(c.running && c.shutdownAt === null, 'a：關閉後 9 秒時仍在執行、尚未開始關閉');
    const exited = await waitUntil(() => c.exit !== null, 26000);
    check(exited, `a：關閉後 ${secs((c.exit ? c.exit.at : Date.now()) - t0)} 秒內程序結束（上限 35 秒）`);
    if (c.shutdownAt !== null) {
      const d = c.shutdownAt - t0;
      check(d >= 9000 && d <= 12000, `a：開始關閉於關閉後 ${secs(d)} 秒（允許 9–12 秒）`);
    } else {
      check(false, 'a：日誌沒有「開始正常關閉」');
    }
    check(c.exit !== null && c.exit.code === 0, `a：結束碼 0（實際 ${c.exit ? c.exit.code : '未結束'}）`);
    check(
      c.exit !== null && c.exit.at - t0 >= 9000,
      `a：不早於關閉後 9 秒結束（實際 ${c.exit ? secs(c.exit.at - t0) : '-'} 秒）`
    );
  });

  // b) 3 秒內重連 → 持續執行。
  await runCase('b-重連', true, async (c) => {
    const ws1 = await c.connect();
    await sleep(300);
    const t0 = await c.closeWs(ws1);
    await sleep(3000 - (Date.now() - t0));
    const ws2 = await c.connect();
    log('b：已於關閉後 3 秒重連');
    await sleep(15000 - (Date.now() - t0));
    check(
      c.running && c.shutdownAt === null,
      `b：第一次關閉後 ${secs(Date.now() - t0)} 秒時仍在執行、沒有開始關閉`
    );
    ws2.close();
  });

  // c) 8 秒時 GET /api/state 延長期限，12 秒重連（已超過原本的 10 秒）→ 持續執行。
  await runCase('c-偵測延長', true, async (c) => {
    const ws1 = await c.connect();
    await sleep(300);
    const t0 = await c.closeWs(ws1);
    await sleep(8000 - (Date.now() - t0));
    const res = await fetch(`http://127.0.0.1:${c.port}/api/state`);
    check(res.status === 200, `c：關閉後 ${secs(Date.now() - t0)} 秒的 GET /api/state 回 200（實際 ${res.status}）`);
    await res.arrayBuffer();
    await sleep(11000 - (Date.now() - t0));
    check(
      c.running && c.shutdownAt === null,
      `c：關閉後 ${secs(Date.now() - t0)} 秒（已過原本的 10 秒）仍在執行`
    );
    await sleep(12000 - (Date.now() - t0));
    const ws2 = await c.connect();
    log('c：已於關閉後約 12 秒重連（GET 後約 4 秒）');
    await sleep(15000 - (Date.now() - t0));
    check(
      c.running && c.shutdownAt === null,
      `c：關閉後 ${secs(Date.now() - t0)} 秒仍在執行、沒有開始關閉`
    );
    ws2.close();
  });

  // d) 沒有旗標：不因連線關閉而結束。
  await runCase('d-無旗標', false, async (c) => {
    const ws = await c.connect();
    await sleep(300);
    const t0 = await c.closeWs(ws);
    await sleep(20000 - (Date.now() - t0));
    check(c.running && c.shutdownAt === null, `d：不帶旗標，關閉後 ${secs(Date.now() - t0)} 秒仍在執行`);
  });

  if (failures.length === 0) {
    console.log('RESULT: PASS');
  } else {
    console.log(`RESULT: FAIL (${failures.length})`);
    process.exitCode = 1;
  }
}

main().catch((e) => {
  console.error(e);
  console.log('RESULT: FAIL (script error)');
  process.exit(1);
});
