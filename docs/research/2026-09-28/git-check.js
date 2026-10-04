// git-check.js：git-review 前端驗收腳本（git-review task 4.2 起；task 4.3 加 diff 分頁內容；
// task 4.4 加 Git Graph 分頁內容；task 4.5 加 commit 詳情、比較與某版本檔案分頁）。
// headless Chrome＋CDP，啟動、收尾、段落代號與「只跑指定段落」的寫法沿用
// docs/research/2026-09-27/files-check.js（本身沿用 docs/research/2026-09-23/visual-check.js）：
// startPreview／startChrome／killTree＋tasklist 收尾判準、「還握著 ChildProcess 且沒觀察到 exit 才
// 終止」的行程所有權模型、parseSegmentArg、打錯段名 exit 2、段落代號加 capability 前綴。
//
// 對 `cockpit --example ui_preview` 逐條驗 openspec/changes/git-review/specs/git-review/spec.md
// 「左欄變更分頁」的三個 scenario（task 4.2）：
//   - 顯示變更並開啟 diff
//   - 新變更自動出現
//   - 不是目前分頁時不讀取
// 與「diff 分頁」的三個 scenario（task 4.3）：
//   - 左右並排呈現
//   - 改檔後更新
//   - 窄視窗不橫向捲動
// 「左欄三個分頁」（file-review「左欄檔案樹」MODIFIED 的三分頁 tablist 本身，含鍵盤操作）不在本檔——
// 那一段跟 git 後端或「變更」面板的內容完全無關，只驗左欄 tablist 的通用機制，files-check.js 已有
// 現成的左欄分頁夾具，不必為了它另外啟動一套機器，見該檔 `segLeftThreeTabs` 段的註解與
// git-check.md「為什麼放在 files-check.js」。
//
// Git Graph 分頁的內容（task 4.4；spec「Git Graph 分頁」）：
//   - 開啟並分批載入
//   - 搜尋跳轉
//   - 分支變更提示
//   - 重畫不影響 Git Graph
// commit 詳情、比較與某版本檔案分頁（task 4.5；spec「commit 詳情與比較」「某版本檔案分頁」）：
//   - 看 commit 的變更並開 diff
//   - 比較兩個 commit
//   - 複製 hash
//   - 看舊版規格
//   - commit 版本不輪詢
//
// 用法（repo 根；先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-09-28/git-check.js                              # 全部段落
//   node docs/research/2026-09-28/git-check.js "self/鷹架,git-review/新變更自動出現"  # 只跑指定段落（逗號分隔）
//   node docs/research/2026-09-28/git-check.js git-review/                  # 以 `<capability>/` 選該前綴的全部段落
// 段落代號拼錯、空字串或只有逗號 → 印 `RESULT: FAIL (段落代號)`、exit 2，不啟動任何行程。
//
// 段落代號：
//   self/鷹架                                    腳本鷹架自我測試（見 segSelfScaffold 註解）
//   self/段落代號                                命令列段落代號驗證
//   git-review/顯示變更並開啟 diff               左欄變更分頁
//   git-review/新變更自動出現                    左欄變更分頁
//   git-review/不是目前分頁時不讀取              左欄變更分頁
//   git-review/重畫不影響變更分頁                左欄變更分頁（fix round 2）
//   git-review/左右並排呈現                      diff 分頁（task 4.3）
//   git-review/改檔後更新                        diff 分頁（task 4.3）
//   git-review/窄視窗不橫向捲動                  diff 分頁（task 4.3）
//   git-review/開啟並分批載入                    Git Graph 分頁（task 4.4）
//   git-review/搜尋跳轉                          Git Graph 分頁（task 4.4）
//   git-review/分支變更提示                      Git Graph 分頁（task 4.4）
//   git-review/搜尋命中後背景載入不拉動捲動      Git Graph 分頁（ui-fixes task 4.5）
//   git-review/有非 commit tag 時不誤報分支變更  Git Graph 分頁（ui-fixes task 4.6）
//   git-review/重畫不影響 Git Graph               Git Graph 分頁（task 4.4）
//   git-review/看 commit 的變更並開 diff          commit 詳情與比較（task 4.5）
//   git-review/比較兩個 commit                    commit 詳情與比較（task 4.5）
//   git-review/commit 詳情檔案清單被截斷         commit 詳情與比較（ui-fixes task 4.7）
//   git-review/兩個 commit 比較的檔案清單被截斷  commit 詳情與比較（ui-fixes task 4.7）
//   git-review/詳情重建後焦點留在對應元素        commit 詳情與比較（ui-fixes task 4.8）
//   git-review/滑鼠觸發的詳情重建不呈現焦點外框  commit 詳情與比較（ui-fixes task 4.8）
//   git-review/複製 hash                          commit 詳情與比較（task 4.5）
//   git-review/看舊版規格                          某版本檔案分頁（task 4.5）
//   git-review/commit 版本不輪詢                   某版本檔案分頁（task 4.5）
//   git-review/隱藏的 Git Graph 不自動載入        code review 缺陷 M1（非目前分頁不自動載入，切回後可繼續）
//   git-review/比較詳情標頭不含 undefined          code review 缺陷 M2（進入比較先設 detailData）
//   git-review/看此版本按鈕鍵盤操作                code review 缺陷 M3（內嵌按鈕的 Enter 不被整列攔走）
//   file-review/還原 git 分頁與變更分頁            分頁還原（task 4.5；放在本檔而非 files-check.js，
//                                                 見下方段落註解）
//
// 預覽資料：ui_preview 啟動時把 cockpit/examples/fixtures/review-repo/ 複製到
// %TEMP%\cockpit-ui-preview-<pid>-<ns>\review-repo（stdout 印 `review-repo: <路徑>`），另建 other-repo；
// 假 pane `win/wJ:p4`（cwd＝review-repo/src）指向 review-repo。工作區既有：`history/staged-change.txt`
// （已暫存 M）、`history/unstaged-change.txt`（未暫存 M）、`history/deleted-in-worktree.txt`（未暫存
// D）、`history/untracked-file.md`（未追蹤 ?）——見 `.superpowers/sdd/tasks/task-3.1-report.md`。
// 需要額外檔案（新變更自動出現段的 `later.md`／填充清單）一律只寫暫存副本，不碰 repo 內的 fixture
// （self/鷹架 另外用雜湊確認 repo 內 fixture 沒被改）。Git Graph 三段（搜尋跳轉、分支變更提示）需要
// 真正新增 commit：一律只對暫存副本操作，寫入前用 `git rev-parse --show-toplevel` 驗證路徑就是
// 暫存副本本身（`verifyTempRepoToplevel()`），不符就丟例外拒絕寫入；搜尋跳轉段用
// `git commit-tree`＋`update-ref` 疊在 HEAD 上開一個新分支（不碰工作區／索引，不會意外把 fixture
// 既有的已暫存修改一併 commit 掉），分支變更提示段依控制端裁決直接用
// `git -c user.name=x -c user.email=x@x -c commit.gpgsign=false commit --allow-empty -m ...`。
//
// 注意：本檔一次只開一個 ui_preview，也不要與 files-check.js／visual-check.js 等同時跑（計時斷言與
// 收尾清查會互相干擾）。開跑前若偵測到有 ui_preview.exe 在跑，或 127.0.0.1:7770 有人 LISTEN，就直接
// 結束（exit 2），不動別人的行程。
'use strict';

const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const crypto = require('node:crypto');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const FIXTURE_SRC = path.join(REPO, 'cockpit', 'examples', 'fixtures', 'review-repo');
const CHROME = process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

const STABLE_PUSH_MS = '600000'; // 背景投影輪替拉長，段落期間靜止（同 files-check.js）
const CHROME_UDD_PREFIX = 'cockpit-chrome-gitcheck-';
const PREVIEW_TEMP_PREFIX = 'cockpit-ui-preview-';
const RUNTIME = 'win';
const PANE_REVIEW = 'wJ:p4'; // cwd＝review-repo/src
const PANE_OTHER = 'wJ:p5'; // cwd＝other-repo（含一個進行中且有衝突的 merge，design D10；conflict.txt）
const UI_TIMEOUT_MS = 5000;

// ---------------------------------------------------------------------------
// 命令列（逐字沿用 files-check.js）
// ---------------------------------------------------------------------------

const ARGV = process.argv.slice(2);
const POSITIONAL = [];
for (const a of ARGV) POSITIONAL.push(a);
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

const failures = [];
let CURRENT_SEGMENT = null;
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push({ segment: CURRENT_SEGMENT, label });
  return !!cond;
}
class SegmentAbort extends Error {}
function need(cond, label) {
  if (!check(cond, label)) throw new SegmentAbort(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
// 輪詢一個 Node 端（非頁面內）的條件，例如 `ctx.net` 這種只存在於腳本行程的記錄——跟 CDP.poll()
// 不同，CDP.poll() 的函式是丟進頁面裡用 Runtime.evaluate 執行的，引用不到 Node 端變數。
async function waitNodeCondition(fn, timeoutMs, intervalMs = 100) {
  const start = Date.now();
  for (;;) {
    if (fn()) return true;
    if (Date.now() - start >= timeoutMs) return false;
    await sleep(intervalMs);
  }
}

// ---------------------------------------------------------------------------
// 行程管理（逐字沿用 files-check.js／visual-check.js）
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

const SPAWNED = [];
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
// CDP（同 files-check.js）
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
  // 真實按鍵（keyDown＋keyUp；ui-fixes task 4.8 起：詳情重建焦點段需要 Tab／Enter 的真實鍵盤輸入）。
  async press(name) {
    const keys = {
      Tab: { key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 },
      Enter: { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' },
    };
    const k = keys[name];
    await this.send('Input.dispatchKeyEvent', { type: 'keyDown', ...k });
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', key: k.key, code: k.code, windowsVirtualKeyCode: k.windowsVirtualKeyCode });
  }
}

async function startChrome(cdpPort, url, label, windowSize = '1536,1024') {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), `${CHROME_UDD_PREFIX}${label.replace(/[^A-Za-z0-9-]/g, '')}-`));
  const chrome = spawn(
    CHROME,
    ['--headless=new', '--lang=zh-TW', '--disable-gpu', '--no-first-run', `--remote-debugging-port=${cdpPort}`, '--remote-allow-origins=*', `--user-data-dir=${udd}`, `--window-size=${windowSize}`, url],
    { stdio: 'ignore', windowsHide: true }
  );
  chrome.on('error', (e) => check(false, `${label} chrome spawn error：${e.message}`));
  SPAWNED.push({ child: chrome, label: `${label}（chrome）`, port: cdpPort });
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
      // Windows 偶爾 EBUSY／EPERM，稍後重試。
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
  const port = pickPort(7970);
  const info = { reviewRepo: null, otherRepo: null };
  const server = spawn(UI_PREVIEW_EXE, [], {
    stdio: ['ignore', 'pipe', 'ignore'],
    windowsHide: true,
    env: { ...process.env, COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`, COCKPIT_PREVIEW_PUSH_MS: STABLE_PUSH_MS, ...envOverrides },
  });
  server.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
  SPAWNED.push({ child: server, label: `${label}（preview）`, port });
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

function writeTemp(preview, rel, content) {
  const p = path.join(preview.reviewRepo, ...rel.split('/'));
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, content);
  return p;
}

function hashTree(dir) {
  const h = crypto.createHash('sha256');
  const walk = (d) => {
    for (const name of fs.readdirSync(d).sort()) {
      const p = path.join(d, name);
      const st = fs.statSync(p);
      h.update(path.relative(dir, p));
      if (st.isDirectory()) walk(p);
      else h.update(fs.readFileSync(p));
    }
  };
  walk(dir);
  return h.digest('hex');
}

// ---------------------------------------------------------------------------
// 在 ui_preview 暫存副本 repo 內寫入（git-review task 4.4：Git Graph 的分批載入／搜尋／分支變更
// 提示三段需要真正新增 commit，光改檔案不夠）。安全機制：每次呼叫前先用 `rev-parse --show-toplevel`
// 驗證目標路徑就是暫存副本本身（design D2「對任何既有 git repo 只讀」——這裡操作的是 ui_preview
// 自己複製出來、本腳本專用的暫存目錄，不是 repo 本體或 WSL 端既有 repo），路徑不符就丟例外拒絕寫入。
// ---------------------------------------------------------------------------

function verifyTempRepoToplevel(repoDir) {
  const r = spawnSync('git', ['-C', repoDir, 'rev-parse', '--show-toplevel'], { encoding: 'utf8' });
  const top = (r.stdout || '').trim();
  if (r.status !== 0 || path.resolve(top).toLowerCase() !== path.resolve(repoDir).toLowerCase()) {
    throw new Error(`安全檢查失敗：git rev-parse --show-toplevel（${JSON.stringify(top)}）與預期的暫存副本路徑（${repoDir}）不符，拒絕寫入`);
  }
}

function gitTemp(repoDir, args, env) {
  verifyTempRepoToplevel(repoDir);
  const r = spawnSync('git', ['-C', repoDir, ...args], { encoding: 'utf8', env: env || process.env });
  if (r.status !== 0) {
    throw new Error(`git ${args.join(' ')} 於 ${repoDir} 失敗：${r.stderr}`);
  }
  return (r.stdout || '').trim();
}

// 疊在 HEAD 上、樹內容全部沿用 HEAD（不碰工作區／索引，因此不會意外把 fixture 既有的已暫存修改
// 一併 commit 掉）的一串新 commit，指到一個新分支：`subjects` 依序成為每個 commit 的完整訊息。
function addChainedEmptyCommits(repoDir, branchName, subjects) {
  const base = gitTemp(repoDir, ['rev-parse', 'HEAD']);
  const tree = gitTemp(repoDir, ['rev-parse', `${base}^{tree}`]);
  const env = { ...process.env, GIT_AUTHOR_NAME: 'git-check', GIT_AUTHOR_EMAIL: 'git-check@invalid', GIT_COMMITTER_NAME: 'git-check', GIT_COMMITTER_EMAIL: 'git-check@invalid' };
  let parent = base;
  subjects.forEach((subject) => {
    parent = gitTemp(repoDir, ['-c', 'user.name=git-check', '-c', 'user.email=git-check@invalid', '-c', 'commit.gpgsign=false', 'commit-tree', tree, '-p', parent, '-m', subject], env);
  });
  gitTemp(repoDir, ['update-ref', `refs/heads/${branchName}`, parent]);
  return parent;
}

// ui-fixes task 4.7：造一個「變更檔案多到後端截斷」的 commit。後端截斷來自執行器的 stdout 位元組上限
// （cockpit-git/src/query.rs 的 CHANGED_FILES_STDOUT_CAP＝4 MiB，commit 詳情與兩點比較共用；不為測試
// 在產品程式加可設定的上限），所以檔案數用實際上限推算：name-status／numstat 每個檔案各佔「路徑長度＋
// 3～5 位元組」，N = 上限 / (路徑長度＋3) 再加餘裕，兩個輸出都一定超過上限。路徑刻意拉長（約 1600
// 字元、四層各約 400）讓需要的檔案數降到數千個，詳情清單不是虛擬清單，檔案數太多會讓 DOM 過重。
// 用一次 `git fast-import` 在暫存副本 repo 疊在 HEAD 上、指到新分支 `branchName`（只建物件與 ref，
// 不動工作區與 index）；commit 時間用現在，確保排在 Git Graph 最前面（fixture 歷史是 2026 年初）。
// 所有檔案共用同一個空 blob，stream 只有路徑本身的大小。
const CHANGED_FILES_STDOUT_CAP = 4 * 1024 * 1024;
function addHugeChangeCommit(repoDir, branchName) {
  const base = gitTemp(repoDir, ['rev-parse', 'HEAD']);
  const dirs = ['a', 'b', 'c'].map((c) => c.repeat(400));
  const pathOf = (i) => `${dirs.join('/')}/${'f'.repeat(380)}${String(i).padStart(6, '0')}`;
  const pathLen = pathOf(0).length;
  const count = Math.ceil(CHANGED_FILES_STDOUT_CAP / (pathLen + 3)) + 50;
  const ts = Math.floor(Date.now() / 1000);
  const ident = `git-check <git-check@invalid> ${ts} +0000`;
  const message = 'git-check: 變更檔案多到被後端截斷的 commit';
  const parts = [`blob\nmark :1\ndata 0\n\n`, `commit refs/heads/${branchName}\nauthor ${ident}\ncommitter ${ident}\ndata ${Buffer.byteLength(message)}\n${message}\nfrom ${base}\n`];
  for (let i = 0; i < count; i += 1) parts.push(`M 100644 :1 ${pathOf(i)}\n`);
  parts.push('\n');
  verifyTempRepoToplevel(repoDir);
  const r = spawnSync('git', ['-C', repoDir, 'fast-import', '--quiet'], { input: parts.join(''), encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (r.status !== 0) throw new Error(`git fast-import 於 ${repoDir} 失敗：${r.stderr}`);
  return { oid: gitTemp(repoDir, ['rev-parse', `refs/heads/${branchName}`]), count, base };
}

// ---------------------------------------------------------------------------
// 頁面端工具：Page.addScriptToEvaluateOnNewDocument 安裝成 window.__gc（重新整理後仍在）。
// ---------------------------------------------------------------------------

function pageHelpers() {
  if (window.__gc) return;
  const txt = (el) => {
    if (!el) return '';
    let t = el.innerText;
    if (typeof t !== 'string') t = el.textContent || '';
    return t.replace(/\s+/g, ' ').trim();
  };
  const visible = (el) => {
    if (!el || !el.isConnected) return false;
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden') return false;
    return el.getClientRects().length > 0;
  };
  const filesRoot = () => document.getElementById('files');
  const leftTablist = () => (filesRoot() ? filesRoot().querySelector('[role="tablist"]') : null);
  const leftTabs = () => (leftTablist() ? Array.from(leftTablist().querySelectorAll('[role="tab"]')) : []);
  const leftTab = (name) => leftTabs().find((t) => txt(t) === name) || null;
  const leftSelected = () => {
    const t = leftTabs().find((x) => x.getAttribute('aria-selected') === 'true');
    return t ? txt(t) : null;
  };
  const changesPanel = () => document.getElementById('changes-panel');
  const changesList = () => (changesPanel() ? changesPanel().querySelector('.files-tree') : null);
  const changesRows = () => (changesList() ? Array.from(changesList().children).filter((el) => el.classList.contains('changes-row')) : []);
  const changesRow = (titlePath) => changesRows().find((r) => r.getAttribute('title') === titlePath) || null;
  const changesGroupOf = (titlePath) => {
    const row = changesRow(titlePath);
    if (!row) return null;
    for (let n = row.previousElementSibling; n; n = n.previousElementSibling) {
      if (n.classList.contains('changes-group-title')) return txt(n);
    }
    return null;
  };
  const changesHeaderText = () => {
    const p = changesPanel();
    if (!p) return '';
    const list = changesList();
    const parts = [];
    const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if (list && list.contains(n)) continue;
      if (!n.parentElement || !visible(n.parentElement)) continue;
      const s = n.textContent.trim();
      if (s) parts.push(s);
    }
    return parts.join(' ');
  };
  const changesButton = (label) => {
    const p = changesPanel();
    if (!p) return null;
    return Array.from(p.querySelectorAll('button')).find((b) => txt(b) === label) || null;
  };
  const changesScroller = () => changesList();
  const reviewRoot = () => document.getElementById('review');
  const reviewTablist = () => (reviewRoot() ? reviewRoot().querySelector('[role="tablist"]') : null);
  const reviewTabs = () => (reviewTablist() ? Array.from(reviewTablist().querySelectorAll('[role="tab"]')) : []);
  const isLive = (t) => /Live Output/.test(txt(t));
  const diffTab = (p, from, to) => reviewTabs().find((t) => t.getAttribute('data-diff-path') === p && t.getAttribute('data-diff-from') === from && t.getAttribute('data-diff-to') === to) || null;
  const graphTab = (rootId) => reviewTabs().find((t) => t.getAttribute('data-graph-root') === rootId) || null;
  const fileTab = (p) => reviewTabs().find((t) => t.getAttribute('data-path') === p) || null;
  const revTab = (p, rev) => reviewTabs().find((t) => t.getAttribute('data-rev-path') === p && t.getAttribute('data-rev') === rev) || null;
  const selectedReviewTab = () => reviewTabs().find((t) => t.getAttribute('aria-selected') === 'true') || null;
  // git-review task 4.3：diff 分頁內容（design D7；spec「diff 分頁」）的量測工具。
  const panelOf = (t) => (t ? document.getElementById(t.getAttribute('aria-controls')) : null);
  // git-review task 4.5：某版本檔案分頁（design D9；spec「某版本檔案分頁」）的量測工具。
  const revPanel = (p, rev) => panelOf(revTab(p, rev));
  const revHost = (panel) => (panel ? panel.querySelector('.file-viewer-host') : null);
  const revStatusText = (panel) => {
    const el = panel ? panel.querySelector('.file-status') : null;
    return el && !el.hidden ? txt(el) : null;
  };
  const revIsStale = (panel) => !!panel && panel.classList.contains('is-stale');
  const revButton = (panel, label) => (panel ? Array.from(panel.querySelectorAll('.file-toolbar button')).find((b) => txt(b) === label) || null : null);
  const revVersionText = (panel) => {
    const el = panel ? panel.querySelector('.diff-toolbar-versions') : null;
    return el ? txt(el) : null;
  };
  const diffPanel = (p, from, to) => panelOf(diffTab(p, from, to));
  const diffGrid = (panel) => (panel ? panel.querySelector('.diff-grid') : null);
  const diffHost = (panel) => (panel ? panel.querySelector('.file-viewer-host') : null);
  // 把格線的扁平子節點（每個非 gap 列 4 個：左行號／左文字／右行號／右文字；gap 列 1 個）還原成
  // 邏輯列，方便逐列斷言（不必在測試碼裡重複格線的扁平化規則）。
  const diffRows = (panel) => {
    const g = diffGrid(panel);
    if (!g) return [];
    const kids = Array.from(g.children);
    const rows = [];
    for (let i = 0; i < kids.length; ) {
      const el = kids[i];
      if (el.classList.contains('diff-gap')) {
        rows.push({ kind: 'gap', text: txt(el) });
        i += 1;
        continue;
      }
      const ln = kids[i];
      const lt = kids[i + 1];
      const rn = kids[i + 2];
      const rt = kids[i + 3];
      rows.push({
        kind: 'row',
        leftLine: txt(ln),
        leftText: txt(lt),
        rightLine: txt(rn),
        rightText: txt(rt),
        leftDel: ln.classList.contains('diff-row-del'),
        leftBlank: ln.classList.contains('diff-row-blank'),
        rightAdd: rn.classList.contains('diff-row-add'),
        rightBlank: rn.classList.contains('diff-row-blank'),
      });
      i += 4;
    }
    return rows;
  };
  const diffToolbarButton = (panel, label) => (panel ? Array.from(panel.querySelectorAll('.diff-toolbar button, .diff-toolbar a')).find((b) => txt(b) === label) || null : null);
  const diffPathText = (panel) => {
    const el = panel ? panel.querySelector('.diff-toolbar-path') : null;
    return el ? txt(el) : null;
  };
  const diffVersionsText = (panel) => {
    const el = panel ? panel.querySelector('.diff-toolbar-versions') : null;
    return el ? txt(el) : null;
  };
  const diffStatusText = (panel) => {
    const el = panel ? panel.querySelector('.file-status') : null;
    return el && !el.hidden ? txt(el) : null;
  };
  const diffIsStale = (panel) => !!panel && panel.classList.contains('is-stale');
  // fix round 2：「重畫不影響變更分頁」段的量測工具（同 files-check.js 的 treeSnapshot／
  // treeCompare 做法，這裡不需要 finderKey——沒有自我測試用的合成 DOM）。projectsHidden()：
  // `[data-region="projects"]`（render.js renderProjectsRegion()）目前是否 hidden。
  let changesKeep = null;
  const changesSnapshot = () => {
    const list = changesList();
    const sc = changesScroller();
    if (!list || !sc) return { error: `list ${!!list}、捲動容器 ${!!sc}` };
    const rows = changesRows();
    changesKeep = { list, sc, rows, descendants: Array.from(list.querySelectorAll('*')), scrollTop: sc.scrollTop };
    return { rows: rows.length, descendants: changesKeep.descendants.length, scrollTop: sc.scrollTop };
  };
  const changesCompare = () => {
    const k = changesKeep;
    if (!k) return { error: '沒有先呼叫 changesSnapshot()' };
    const rows = changesRows();
    const list = changesList();
    const desc = list ? Array.from(list.querySelectorAll('*')) : [];
    const sc = changesScroller();
    return {
      sameList: list === k.list && k.list.isConnected,
      sameRows: rows.length === k.rows.length && rows.every((r, i) => r === k.rows[i]),
      sameDescendants: desc.length === k.descendants.length && desc.every((n, i) => n === k.descendants[i]),
      descendants: desc.length,
      scSame: sc === k.sc && k.sc.isConnected,
      scrollTop: sc ? sc.scrollTop : null,
      expectScrollTop: k.scrollTop,
    };
  };
  const projectsHidden = () => {
    const el = document.querySelector('[data-region="projects"]');
    return el ? el.hidden : null;
  };
  // git-review task 4.4：Git Graph 分頁（design D8／D9；spec「Git Graph 分頁」）的量測工具。
  const graphPanelOf = (rootId) => panelOf(graphTab(rootId));
  const graphRows = (panel) => (panel ? Array.from(panel.querySelectorAll('.graph-row')) : []);
  const graphRowByOid = (panel, oid) => graphRows(panel).find((r) => r.getAttribute('data-oid') === oid) || null;
  const graphScroller = (panel) => (panel ? panel.querySelector('.graph-scroll') : null);
  const graphSearchInput = (panel) => (panel ? panel.querySelector('.graph-search-input') : null);
  const graphSearchCount = (panel) => {
    const el = panel ? panel.querySelector('.graph-search-count') : null;
    return el ? txt(el) : null;
  };
  const graphBanner = (panel) => (panel ? panel.querySelector('.action-banner') : null);
  const graphButtonByAction = (panel, action) => (panel ? panel.querySelector(`[data-action="${action}"]`) : null);
  const graphFilterCheckbox = (panel, refName) => {
    const pop = panel ? panel.querySelector('.graph-filter-popover') : null;
    return pop ? Array.from(pop.querySelectorAll('input[type=checkbox]')).find((cb) => cb.value === refName) || null : null;
  };
  // 同 changesSnapshot()／changesCompare()（fix round 2）：驗「重畫不影響 Git Graph」用。
  let graphKeep = null;
  const graphSnapshot = (rootId) => {
    const panel = graphPanelOf(rootId);
    const sc = graphScroller(panel);
    const list = panel ? panel.querySelector('.graph-listbox') : null;
    if (!panel || !sc || !list) return { error: `panel ${!!panel}、捲動容器 ${!!sc}、清單 ${!!list}` };
    const rows = graphRows(panel);
    graphKeep = { list, sc, rows, descendants: Array.from(list.querySelectorAll('*')), scrollTop: sc.scrollTop };
    return { rows: rows.length, descendants: graphKeep.descendants.length, scrollTop: sc.scrollTop };
  };
  const graphCompare = (rootId) => {
    const k = graphKeep;
    if (!k) return { error: '沒有先呼叫 graphSnapshot()' };
    const panel = graphPanelOf(rootId);
    const list = panel ? panel.querySelector('.graph-listbox') : null;
    const rows = graphRows(panel);
    const desc = list ? Array.from(list.querySelectorAll('*')) : [];
    const sc = graphScroller(panel);
    return {
      sameList: list === k.list && k.list.isConnected,
      sameRows: rows.length === k.rows.length && rows.every((r, i) => r === k.rows[i]),
      sameDescendants: desc.length === k.descendants.length && desc.every((n, i) => n === k.descendants[i]),
      descendants: desc.length,
      scSame: sc === k.sc && k.sc.isConnected,
      scrollTop: sc ? sc.scrollTop : null,
      expectScrollTop: k.scrollTop,
    };
  };
  // git-review task 4.5：commit 詳情與比較（design 控制端裁決；spec「commit 詳情與比較」）的量測工具。
  const graphRowBySubject = (panel, needle) => graphRows(panel).find((r) => (r.querySelector('.graph-subject') ? txt(r.querySelector('.graph-subject')).includes(needle) : false)) || null;
  const commitDetailWrap = (panel) => (panel ? panel.querySelector('.commit-detail') : null);
  const commitDetailText = (panel) => {
    const w = commitDetailWrap(panel);
    return w ? txt(w) : null;
  };
  const commitDetailButton = (panel, label) => {
    const w = commitDetailWrap(panel);
    return w ? Array.from(w.querySelectorAll('button')).find((b) => txt(b) === label) || null : null;
  };
  const commitDetailFileRow = (panel, path) => {
    const w = commitDetailWrap(panel);
    return w ? Array.from(w.querySelectorAll('.commit-detail-file-row')).find((r) => r.getAttribute('title') === path) || null : null;
  };
  const commitDetailFilePaths = (panel) => {
    const w = commitDetailWrap(panel);
    return w ? Array.from(w.querySelectorAll('.commit-detail-file-row')).map((r) => r.getAttribute('title')) : [];
  };
  const commitDetailToggle = (panel, label) => {
    const b = commitDetailButton(panel, label);
    return b ? { hidden: b.hidden, pressed: b.getAttribute('aria-pressed') } : null;
  };
  const copyFeedbackText = (panel) => {
    const els = panel ? Array.from(panel.querySelectorAll('.file-status[aria-live="polite"]')) : [];
    const el = els.find((e) => !e.hidden);
    return el ? txt(el) : null;
  };
  window.__gc = {
    txt,
    visible,
    leftTab,
    leftSelected,
    changesPanel,
    changesList,
    changesRows,
    changesRow,
    changesGroupOf,
    changesHeaderText,
    changesButton,
    changesScroller,
    reviewTabs,
    isLive,
    diffTab,
    graphTab,
    fileTab,
    revTab,
    selectedReviewTab,
    diffPanel,
    diffGrid,
    diffHost,
    diffRows,
    diffToolbarButton,
    diffPathText,
    diffVersionsText,
    diffStatusText,
    diffIsStale,
    changesSnapshot,
    changesCompare,
    projectsHidden,
    graphPanelOf,
    graphRows,
    graphRowByOid,
    graphScroller,
    graphSearchInput,
    graphSearchCount,
    graphBanner,
    graphButtonByAction,
    graphFilterCheckbox,
    graphSnapshot,
    graphCompare,
    revPanel,
    revHost,
    revStatusText,
    revIsStale,
    revButton,
    revVersionText,
    graphRowBySubject,
    commitDetailWrap,
    commitDetailText,
    commitDetailButton,
    commitDetailFileRow,
    commitDetailFilePaths,
    commitDetailToggle,
    copyFeedbackText,
  };
}

// classify()：多加 git 端點各動作的分類（自我測試與「不是目前分頁時不讀取」段用）。
function classify(url) {
  let p;
  try {
    p = new URL(url).pathname;
  } catch {
    return 'other';
  }
  const g = /^\/api\/git\/[^/]+\/[^/]+\/(status|refs|log|commit|changes|merge-base|diff|meta|blob|render)(\/|$)/.exec(p);
  if (g) return 'git_' + g[1];
  if (/^\/api\/runtimes\/[^/]+\/panes\/[^/]+\/root$/.test(p)) return 'root';
  if (p === '/api/state') return 'state';
  return 'other';
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

async function recordNetwork(cdp) {
  const reqs = [];
  const byKey = new Map();
  cdp.onEvent('Network.requestWillBeSent', (p, sid) => {
    const r = { key: `${sid || ''}|${p.requestId}`, url: p.request.url, kind: classify(p.request.url), at: Date.now(), status: null };
    byKey.set(r.key, r);
    reqs.push(r);
  });
  cdp.onEvent('Network.responseReceived', (p, sid) => {
    const r = byKey.get(`${sid || ''}|${p.requestId}`);
    if (r) r.status = p.response.status;
  });
  await cdp.send('Network.enable');
  return { reqs, since: (t, kind) => reqs.filter((r) => r.at >= t && (!kind || r.kind === kind)) };
}

// ---------------------------------------------------------------------------
// 段落共用：啟動 preview＋chrome、導覽、等首份投影；收尾
// ---------------------------------------------------------------------------

async function waitForFirstProjection(cdp, previewPort, label) {
  const start = Date.now();
  while (Date.now() - start < 8000) {
    const s = await cdp
      .eval(`(() => { const lamp = document.querySelector('[data-region="topbar"] [data-runtime]');
        const v = document.getElementById('version'); return { lamp: !!lamp, v: v ? (v.hasAttribute('data-state-version') ? 'v' + v.getAttribute('data-state-version') : '') : null }; })()`)
      .catch(() => null);
    if (s && s.lamp && s.v) {
      const expected = await fetch(`http://127.0.0.1:${previewPort}/api/state`).then((r) => r.json()).catch(() => null);
      if (expected && s.v === `v${expected.version}`) {
        check(true, label || '第一份真投影已畫出（頂列有 [data-runtime] 且 #version 的 data-state-version 等於 /api/state）');
        return true;
      }
    }
    await sleep(50);
  }
  need(false, `逾時：${label || '第一份真投影已畫出'}`);
  return false;
}

async function openCockpit(label, opts = {}) {
  const ctx = { label, preview: null, chrome: null };
  try {
    ctx.preview = await startPreview(opts.env || {}, `preview-${label}`);
    if (opts.beforeLoad) opts.beforeLoad(ctx.preview);
    ctx.chrome = await startChrome(pickPort(19410, [ctx.preview.port]), 'about:blank', label, opts.windowSize);
    ctx.cdp = ctx.chrome.cdp;
    ctx.origin = `http://127.0.0.1:${ctx.preview.port}`;
    await ctx.cdp.send('Page.enable');
    await ctx.cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: `(${pageHelpers.toString()})();` });
    ctx.net = await recordNetwork(ctx.cdp);
    ctx.console = await recordConsole(ctx.cdp);
    if (opts.beforeNavigate) await opts.beforeNavigate(ctx);
    ctx.loadedAt = Date.now();
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

async function apiJson(ctx, p) {
  const r = await fetch(`${ctx.origin}${p}`);
  const body = await r.json().catch(() => null);
  return { status: r.status, body };
}
async function rootInfo(ctx, pane) {
  const r = await apiJson(ctx, `/api/runtimes/${RUNTIME}/panes/${encodeURIComponent(pane)}/root`);
  need(r.status === 200 && r.body && r.body.root_id, `根目錄查詢 ${RUNTIME}/${pane} 回 200 並帶 root_id（實際 ${r.status} ${JSON.stringify(r.body)}）`);
  return r.body;
}

async function contractDump(ctx) {
  const c = await ctx.cdp
    .run(() => (window.__gc ? { leftTabs: window.__gc.leftTab ? true : false, changes: !!window.__gc.changesPanel(), rows: window.__gc.changesRows().length, tabs: window.__gc.reviewTabs().map((t) => t.getAttribute('data-diff-path') || t.getAttribute('data-graph-root') || t.textContent) } : { helpers: false }))
    .catch((e) => ({ error: e.message }));
  return JSON.stringify(c);
}

async function selectPane(ctx, pane) {
  const sel = `.pane-row[data-runtime="${RUNTIME}"][data-pane="${pane}"]`;
  need(await ctx.cdp.clickEl((s) => document.querySelector(s), [sel], `pane 列 ${RUNTIME}/${pane}`), `點 pane 列 ${RUNTIME}/${pane}`);
  const ok = await ctx.cdp.poll(
    (s) => {
      const r = document.querySelector(s);
      return !!r && r.classList.contains('selected');
    },
    [sel],
    UI_TIMEOUT_MS
  );
  if (!ok && ctx.console) {
    log(`診斷：console/exception 記錄 ${JSON.stringify(ctx.console)}`);
  }
  need(!!ok, `pane 列 ${RUNTIME}/${pane} 出現選定標示`);
}

async function switchLeftTab(ctx, name) {
  const found = await ctx.cdp.poll((n) => !!window.__gc.leftTab(n), [name], UI_TIMEOUT_MS);
  if (!found) {
    need(false, `找不到左欄分頁「${name}」；目前 DOM：${await contractDump(ctx)}`);
  }
  need(await ctx.cdp.clickEl((n) => window.__gc.leftTab(n), [name], `左欄分頁「${name}」`), `點左欄分頁「${name}」`);
  const sel = await ctx.cdp.poll(
    (n) => {
      const t = window.__gc.leftTab(n);
      return !!t && t.getAttribute('aria-selected') === 'true';
    },
    [name],
    UI_TIMEOUT_MS
  );
  need(!!sel, `左欄分頁「${name}」成為目前分頁（aria-selected="true"）`);
}

async function openChanges(ctx, pane = PANE_REVIEW) {
  await selectPane(ctx, pane);
  await switchLeftTab(ctx, '變更');
}

async function waitChangesRow(ctx, titlePath, timeoutMs = UI_TIMEOUT_MS) {
  const ok = await ctx.cdp.poll((p) => !!window.__gc.changesRow(p) && window.__gc.visible(window.__gc.changesRow(p)), [titlePath], timeoutMs);
  if (!ok) {
    need(false, `「變更」清單應該出現可見的列 title="${titlePath}"；目前 DOM：${await contractDump(ctx)}`);
  }
}

// ---------------------------------------------------------------------------
// self/鷹架
// ---------------------------------------------------------------------------

// 驗本檔自己的鷹架（preview 啟動、pane 根目錄、狀態端點形狀符合 task 3.1 fixture 的既定內容、網路
// 分類對 git 端點生效），不是在驗前端——這是「先寫測試」的自我檢查，前提不成立的話後面的 scenario
// 段全部不可信。
async function segSelfScaffold() {
  const fixtureHashBefore = hashTree(FIXTURE_SRC);
  let ctx = null;
  try {
    ctx = await openCockpit('self', {});
    const { cdp, preview } = ctx;
    check(!!preview.reviewRepo && fs.existsSync(path.join(preview.reviewRepo, 'README.md')), `讀到 ui_preview 印出的 review-repo 暫存路徑且含 README.md（${preview.reviewRepo}）`);
    check(path.basename(preview.tempRoot).startsWith(PREVIEW_TEMP_PREFIX) && !path.resolve(preview.reviewRepo).toLowerCase().startsWith(REPO.toLowerCase()), `暫存副本在 repo 之外、目錄名以 ${PREVIEW_TEMP_PREFIX} 開頭（${preview.tempRoot}）`);

    const root = await rootInfo(ctx, PANE_REVIEW);
    check(root.name === 'review-repo' && root.is_git === true, `${PANE_REVIEW} 的根目錄＝review-repo、is_git 為 true（實際 ${JSON.stringify(root)}）`);

    const status = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/status`);
    need(status.status === 200 && status.body && status.body.branch && Array.isArray(status.body.entries), `狀態端點回 200 並帶 branch／entries（實際 ${status.status} ${JSON.stringify(status.body)}）`);
    check(status.body.branch.head === 'main', `branch.head 為 main（實際 ${JSON.stringify(status.body.branch)}）`);
    const byPath = new Map(status.body.entries.map((e) => [`${e.group}:${e.path}`, e]));
    check(byPath.get('staged:history/staged-change.txt') && byPath.get('staged:history/staged-change.txt').status === 'M', 'history/staged-change.txt 在 staged 組、狀態 M（task 3.1 fixture）');
    check(byPath.get('unstaged:history/unstaged-change.txt') && byPath.get('unstaged:history/unstaged-change.txt').status === 'M', 'history/unstaged-change.txt 在 unstaged 組、狀態 M（task 3.1 fixture）');
    check(byPath.get('unstaged:history/deleted-in-worktree.txt') && byPath.get('unstaged:history/deleted-in-worktree.txt').status === 'D', 'history/deleted-in-worktree.txt 在 unstaged 組、狀態 D（task 3.1 fixture）');
    check(byPath.get('untracked:history/untracked-file.md') && byPath.get('untracked:history/untracked-file.md').status === '?', 'history/untracked-file.md 在 untracked 組、狀態 ?（task 3.1 fixture）');

    // 網路分類：git 狀態端點被分類為 git_status。
    const t0 = Date.now();
    await cdp.run(async (rid) => {
      await fetch('/api/git/win/' + rid + '/status');
      return true;
    }, root.root_id);
    await sleep(300);
    const recent = ctx.net.since(t0);
    check(recent.some((r) => r.kind === 'git_status' && r.status === 200), '網路記錄：git 狀態端點被分類為 git_status、status 200');
  } finally {
    await closeCockpit(ctx);
  }
  check(hashTree(FIXTURE_SRC) === fixtureHashBefore, 'repo 內的 fixture（cockpit/examples/fixtures/review-repo）內容沒有被改');
  if (ctx && ctx.preview) check(!fs.existsSync(ctx.preview.tempRoot), `ui_preview 暫存副本目錄已不存在（${ctx.preview.tempRoot}）`);
  if (ctx && ctx.chrome) check(!fs.existsSync(ctx.chrome.udd), 'Chrome user-data-dir 已刪除');
  check(runningUiPreviewPids().length === 0, '沒有殘留的 ui_preview.exe');
  check(ourChromePids().length === 0, `沒有殘留的 headless Chrome（user-data-dir 含 ${CHROME_UDD_PREFIX}）`);
}

// ---------------------------------------------------------------------------
// self/段落代號
// ---------------------------------------------------------------------------

async function segSelfSegmentArg() {
  const known = SEGMENTS.map((s) => s.code);
  const cases = [
    { arg: undefined, ok: true, codes: null },
    { arg: 'self/鷹架', ok: true, codes: ['self/鷹架'] },
    { arg: ' git-review/新變更自動出現 , self/鷹架 ', ok: true, codes: ['git-review/新變更自動出現', 'self/鷹架'] },
    { arg: 'git-review/', ok: true, codes: known.filter((c) => c.startsWith('git-review/')) },
    { arg: 'git-review/新變更自動出現x', ok: false, mention: 'git-review/新變更自動出現x' },
    { arg: 'nope/', ok: false, mention: 'nope/' },
    { arg: '', ok: false },
    { arg: ',', ok: false },
  ];
  for (const c of cases) {
    const r = parseSegmentArg(c.arg, known);
    const good = r.ok === c.ok && (c.ok ? JSON.stringify(r.codes) === JSON.stringify(c.codes) : typeof r.message === 'string' && (!c.mention || r.message.includes(c.mention)));
    check(good, `parseSegmentArg(${JSON.stringify(c.arg)}) → ${c.ok ? '通過' : '拒絕'}（實際 ${JSON.stringify(r).slice(0, 160)}）`);
  }
  check(
    known.filter((c) => c.startsWith('git-review/')).length === 27,
    'git-review/ 前綴恰好選中 27 段（task 4.2 三段＋fix round 2 一段＋task 4.3 三段＋目視驗收缺陷修正兩段＋task 4.4 四段＋task 4.5 五段＋code review 缺陷 M1/M2/M3 三段＋ui-fixes task 4.5／4.6 兩段＋ui-fixes task 4.7 兩段＋ui-fixes task 4.8 兩段）'
  );
  for (const arg of ['git-review/不存在', '']) {
    const r = spawnSync(process.execPath, [__filename, arg], { encoding: 'utf8', timeout: 30000, windowsHide: true });
    const out = `${r.stdout || ''}${r.stderr || ''}`;
    check(
      r.status === 2 && !out.includes('RESULT: PASS') && out.includes('RESULT: FAIL (段落代號)') && (arg === '' || out.includes(arg)) && !out.includes('啟動'),
      `node git-check.js ${JSON.stringify(arg)}：exit 2、不印 PASS、指出問題、不啟動行程（exit ${r.status}；輸出 ${JSON.stringify(out.trim().slice(0, 160))}）`
    );
  }
}

// ---------------------------------------------------------------------------
// git-review：左欄變更分頁
// ---------------------------------------------------------------------------

// GIVEN 已選定 pane，其 repo 的 history/unstaged-change.txt 有未暫存修改 WHEN 切到「變更」分頁並點
// history/unstaged-change.txt THEN 「變更」組列出它；分頁區出現它的 diff 分頁（暫存區 → 工作區）並
// 成為目前分頁。額外驗（自我驗證，非 spec 逐字要求）：按「Git Graph」開／切到該 repo 的 Git Graph 分頁。
async function segShowChangesAndOpenDiff() {
  await withCockpit('show-open-diff', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await openChanges(ctx);
    await waitChangesRow(ctx, 'history/unstaged-change.txt');

    const header = await ctx.cdp.run(() => window.__gc.changesHeaderText());
    check(header.includes('review-repo') && header.includes('main'), `「變更」面板頂端顯示根目錄 name「review-repo」與分支「main」（實際「${header}」）`);

    // fix round 1（控制端設計審核）：1536 寬時左欄（design D2 三欄版面的固定寬度）曾經把根目錄
    // name／runtime／分支資訊擠到跟「Git Graph」「重新整理」按鈕重疊。標頭改成兩行後，兩個寬度下
    // 根目錄 name 元素都要可見、且跟按鈕列（.changes-head-actions）的 bounding rect 不重疊。
    for (const [w, h] of [
      [1536, 1024],
      [1100, 900],
    ]) {
      await ctx.cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
      await sleep(200);
      const layout = await ctx.cdp.eval(`(() => {
        const R = (r) => ({ left: r.left, right: r.right, top: r.top, bottom: r.bottom });
        const panel = window.__gc.changesPanel();
        const name = panel ? panel.querySelector('.files-root-name') : null;
        const actions = panel ? panel.querySelector('.changes-head-actions') : null;
        if (!name || !actions) return null;
        const nr = name.getBoundingClientRect();
        const ar = actions.getBoundingClientRect();
        const overlap = !(nr.right <= ar.left || nr.left >= ar.right || nr.bottom <= ar.top || nr.top >= ar.bottom);
        return { visible: window.__gc.visible(name), overlap, name: R(nr), actions: R(ar) };
      })()`);
      check(!!layout && layout.visible && !layout.overlap, `fix round 1：寬 ${w} 時「變更」面板頭部根目錄 name 元素可見且與按鈕列不重疊（實際 ${JSON.stringify(layout)}）`);
    }
    await ctx.cdp.send('Emulation.clearDeviceMetricsOverride', {});
    await sleep(200);

    const group = await ctx.cdp.run((p) => window.__gc.changesGroupOf(p), 'history/unstaged-change.txt');
    check(group === '變更（2）', `history/unstaged-change.txt 在「變更」組（實際「${group}」——unstaged 組固定含 unstaged-change.txt 與 deleted-in-worktree.txt 共 2 筆）`);

    const staged = await ctx.cdp.run((p) => window.__gc.changesGroupOf(p), 'history/staged-change.txt');
    check(staged === '已暫存（1）', `history/staged-change.txt 在「已暫存」組（實際「${staged}」）`);
    const untracked = await ctx.cdp.run((p) => window.__gc.changesGroupOf(p), 'history/untracked-file.md');
    check(untracked === '未追蹤（1）', `history/untracked-file.md 在「未追蹤」組（實際「${untracked}」）`);

    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['history/unstaged-change.txt'], '「變更」清單的 history/unstaged-change.txt 列'),
      '點「變更」清單的 history/unstaged-change.txt 列'
    );
    const opened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.diffTab(p, 'INDEX', 'WORKTREE');
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/unstaged-change.txt'],
      UI_TIMEOUT_MS
    );
    if (!opened) {
      need(false, `點列後應該出現並選定 history/unstaged-change.txt 的 diff 分頁（暫存區 → 工作區）；目前 DOM：${await contractDump(ctx)}`);
    }
    const tabInfo = await ctx.cdp.run(
      (p) => {
        const t = window.__gc.diffTab(p, 'INDEX', 'WORKTREE');
        return { label: window.__gc.txt(t), title: t.title };
      },
      'history/unstaged-change.txt'
    );
    check(tabInfo.label.includes('unstaged-change.txt') && tabInfo.label.includes('工作區'), `diff 分頁標籤含檔名與「工作區」（實際「${tabInfo.label}」）`);
    check(tabInfo.title.includes('history/unstaged-change.txt') && tabInfo.title.includes('review-repo'), `diff 分頁 title 含完整路徑與根目錄名稱（實際「${tabInfo.title}」）`);

    // 額外驗證：Git Graph 按鈕開／切到該 repo 的 graph 分頁（不是本段 spec 逐字要求，但同一個面板順手驗）。
    need(await ctx.cdp.clickEl(() => window.__gc.changesButton('Git Graph'), [], '「Git Graph」按鈕'), '點「Git Graph」按鈕');
    const graphOpened = await ctx.cdp.poll(
      (rid) => {
        const t = window.__gc.graphTab(rid);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    check(!!graphOpened, `按「Git Graph」後出現並選定該 repo 的 Git Graph 分頁；目前 DOM：${graphOpened ? '' : await contractDump(ctx)}`);
  });
}

// GIVEN 「變更」分頁為左欄目前分頁 WHEN 在 repo 新增未追蹤檔案 later.md THEN 3 秒內 later.md 出現在
// 「未追蹤」組，清單捲動位置不變。先寫入一批填充檔案讓清單真的可以捲動（否則「捲動位置不變」在小清單
// 上是平凡地成立，驗不到什麼）。
async function segNewChangeAppears() {
  await withCockpit(
    'new-change',
    {
      beforeLoad: (preview) => {
        for (let i = 1; i <= 80; i += 1) {
          writeTemp(preview, `history/git-check-filler-${String(i).padStart(2, '0')}.md`, `filler ${i}\n`);
        }
      },
    },
    async (ctx) => {
      await openChanges(ctx);
      await waitChangesRow(ctx, 'history/git-check-filler-01.md');

      const scrollInfo = await ctx.cdp.run(() => {
        const sc = window.__gc.changesScroller();
        if (!sc) return null;
        sc.scrollTop = Math.min(20, sc.scrollHeight - sc.clientHeight);
        return { scrollTop: sc.scrollTop, max: sc.scrollHeight - sc.clientHeight };
      });
      need(!!scrollInfo && scrollInfo.max > 0 && scrollInfo.scrollTop > 0, `前置：填充檔案讓「變更」清單可以捲動並捲到非 0 位置（${JSON.stringify(scrollInfo)}）`);
      await sleep(300);
      const before = await ctx.cdp.run(() => window.__gc.changesScroller().scrollTop);

      writeTemp(ctx.preview, 'later.md', 'later\n');
      const t0 = Date.now();
      const shown = await ctx.cdp.poll((p) => !!window.__gc.changesRow(p), ['later.md'], 3000);
      const elapsed = Date.now() - t0;
      check(!!shown, `3 秒內 later.md 出現在清單中（${shown ? `${elapsed} ms` : '逾時'}）`);
      const group = await ctx.cdp.run((p) => window.__gc.changesGroupOf(p), 'later.md');
      check(group === '未追蹤（82）', `later.md 出現在「未追蹤」組（實際「${group}」；80 個填充檔案＋原有 1 個（untracked-file.md）＋later.md＝82）`);
      const after = await ctx.cdp.run(() => window.__gc.changesScroller().scrollTop);
      check(Math.abs(after - before) <= 1, `清單捲動位置不變（${before} → ${after}）`);
    }
  );
}

// GIVEN 左欄目前為「檔案」分頁 WHEN 觀察 10 秒 THEN 服務沒有收到任何狀態查詢。前置另外確認「切到
// 『變更』分頁確實會查詢」，證明計數器量得到 git_status 請求（不是分類邏輯本身就量不到而假陽性通過）。
async function segNoStatusWhenNotCurrent() {
  await withCockpit('no-status', {}, async (ctx) => {
    await selectPane(ctx, PANE_REVIEW);
    await switchLeftTab(ctx, '檔案');
    const t0 = Date.now();
    await sleep(10000);
    const duringFiles = ctx.net.since(t0, 'git_status');
    check(duringFiles.length === 0, `左欄為「檔案」分頁期間，服務沒有收到任何 git 狀態查詢（實際 ${duringFiles.length} 次：${JSON.stringify(duringFiles.map((r) => r.url))}）`);

    const t1 = Date.now();
    await switchLeftTab(ctx, '變更');
    const gotStatus = await waitNodeCondition(() => ctx.net.since(t1, 'git_status').length > 0, UI_TIMEOUT_MS);
    need(!!gotStatus, `前置：切到「變更」分頁後確實會發出 git 狀態查詢（證明上面的計數器量得到；${JSON.stringify(ctx.net.since(t1, 'git_status'))}）`);
  });
}

// GIVEN 左欄目前分頁為「變更」、清單已捲動到非 0 位置、焦點停在某一列 WHEN ui_preview 觸發至少
// 3 次整頁重畫（投影推送）THEN Project 清單仍隱藏、「變更」面板的清單 DOM 節點沒被換掉、捲動位置
// 與焦點不變。fix round 2（控制端在 4.3 截圖發現）：render.js 的 renderProjectsRegion() 原本用
// `leftTab === "files"` 決定 Project 清單是否 hidden（file-review task 4.1 時只有兩個分頁的
// 二選一寫法），git-review task 4.2 加「變更」分頁後沒有同步改成三選一，導致「變更」分頁被誤判為
// 「不是 files 所以要顯示」，整頁重畫後 Project 清單跑出來、跟「變更」面板並存（違反 spec
// git-review「左欄變更分頁」「整頁重畫不得改變此分頁的內容」）。
async function segRepaintKeepsChanges() {
  await withCockpit(
    'changes-repaint',
    {
      env: { COCKPIT_PREVIEW_PUSH_MS: '100' },
      beforeLoad: (preview) => {
        for (let i = 1; i <= 30; i += 1) {
          writeTemp(preview, `history/git-check-repaint-${String(i).padStart(2, '0')}.md`, `repaint filler ${i}\n`);
        }
      },
    },
    async (ctx) => {
      await openChanges(ctx);
      await waitChangesRow(ctx, 'history/git-check-repaint-01.md');

      const setup = await ctx.cdp.run(() => {
        const sc = window.__gc.changesScroller();
        if (!sc) return { error: '找不到「變更」清單的捲動容器' };
        sc.scrollTop = Math.min(20, sc.scrollHeight - sc.clientHeight);
        const target = window.__gc.changesRow('history/staged-change.txt');
        if (!target) return { error: '找不到 history/staged-change.txt 列' };
        target.focus({ preventScroll: true });
        window.__gcFocusTarget = target;
        return { scrollTop: sc.scrollTop, max: sc.scrollHeight - sc.clientHeight, focused: document.activeElement === target };
      });
      need(!!setup && !setup.error && setup.scrollTop > 0 && setup.focused, `前置：「變更」清單捲到非 0 位置、焦點在 history/staged-change.txt 列（${JSON.stringify(setup)}）`);

      const snap = await ctx.cdp.run(() => window.__gc.changesSnapshot());
      need(!snap.error, `前置：記下「變更」清單、每一列與列內所有子孫節點、捲動容器（${JSON.stringify(snap)}）`);

      // 不在這裡先驗一次「Project 是否隱藏」再當前置條件：這個 bug 的表現正是「切到『變更』分頁
      // 後，只要發生一次整頁重畫就會冒出來」（fix round 2 根因：render.js 用 `leftTab === "files"`
      // 誤判），把它當 need() 前置條件只會讓段落在還沒驗到「重畫幾次後」就提早中止，看不出後面
      // DOM／捲動／焦點是否也受影響；直接進下面「至少 3 次重畫」的迴圈，用 check() 驗最終狀態。
      const v0 = await ctx.cdp.run(() => document.getElementById('version').getAttribute('data-state-version'));
      let repaints = 0;
      let lastV = v0;
      const start = Date.now();
      while (Date.now() - start < 5000 && repaints < 3) {
        await sleep(100);
        const v = await ctx.cdp.run(() => document.getElementById('version').getAttribute('data-state-version'));
        if (v !== lastV) {
          repaints += 1;
          lastV = v;
        }
      }
      need(repaints >= 3, `前置：期間至少發生 3 次整頁重畫（#version 的 data-state-version 變化 ${repaints} 次）`);

      const projectsHiddenAfter = await ctx.cdp.run(() => window.__gc.projectsHidden());
      check(
        projectsHiddenAfter === true,
        `重畫 ${repaints} 次後 Project 清單仍隱藏（實際 hidden=${projectsHiddenAfter}；spec git-review「左欄變更分頁」：整頁重畫不得改變此分頁的內容）`
      );

      const after = await ctx.cdp.run(() => window.__gc.changesCompare());
      check(
        after.sameList && after.sameRows && after.sameDescendants,
        `「變更」清單的 DOM 節點沒有被換掉（list ${after.sameList}、列 ${after.sameRows}、列內子孫 ${after.sameDescendants}；${after.descendants} 個節點）`
      );
      check(after.scSame, `捲動容器仍是原本那一個且仍在頁面上（${JSON.stringify({ scSame: after.scSame })}）`);
      check(after.scrollTop !== null && Math.abs(after.scrollTop - after.expectScrollTop) <= 1, `捲動位置不變（${after.expectScrollTop} → ${after.scrollTop}）`);
      const focus = await ctx.cdp.run(() => ({
        same: document.activeElement === window.__gcFocusTarget,
        active: document.activeElement ? document.activeElement.getAttribute('title') || document.activeElement.tagName : null,
      }));
      check(focus.same, `焦點仍在原本的列上（實際 ${focus.active}）`);
    }
  );
}

// ---------------------------------------------------------------------------
// git-review：diff 分頁（git-review task 4.3；design D7）
// ---------------------------------------------------------------------------

// 切回已開啟的 diff 分頁（工具列的「看左側版本」「看右側版本」「開啟檔案」都會切到別的分頁，測完
// 一個動作要切回來才能測下一個）。
async function reselectDiffTab(ctx, p, from, to) {
  need(
    await ctx.cdp.clickEl((path, f, t) => window.__gc.diffTab(path, f, t), [p, from, to], `切回 ${p} 的 diff 分頁（${from} → ${to}）`),
    `切回 ${p} 的 diff 分頁（${from} → ${to}）`
  );
  const sel = await ctx.cdp.poll(
    (path, f, t) => {
      const tab = window.__gc.diffTab(path, f, t);
      return !!tab && tab.getAttribute('aria-selected') === 'true';
    },
    [p, from, to],
    UI_TIMEOUT_MS
  );
  need(!!sel, `diff 分頁（${from} → ${to}）重新成為目前分頁`);
}

// GIVEN history/unstaged-change.txt 第 1 行（檔案僅 1 行）被改寫 WHEN 開啟其 diff 分頁 THEN 同一列
// 左側為舊內容（刪除底色）、右側為新內容（新增底色），兩側行號相同；並額外驗證（同一份 spec
// Requirement 的其餘規則）：路徑／版本工具列文字、delete 列（deleted-in-worktree.txt）的右側空白、
// add 列＋gap 列（docs/design.md 兩處分散插入）、工具列四個動作（開啟檔案／VS Code／看左側版本／
// 看右側版本）。
async function segSideBySideRendering() {
  await withCockpit('side-by-side', {}, async (ctx) => {
    await openChanges(ctx);
    await waitChangesRow(ctx, 'history/unstaged-change.txt');

    // --- 1. change 列：同一列左右行號相同，左刪除底色、右新增底色 ---
    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['history/unstaged-change.txt'], '「變更」清單的 history/unstaged-change.txt 列'),
      '點「變更」清單的 history/unstaged-change.txt 列'
    );
    const opened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.diffTab(p, 'INDEX', 'WORKTREE');
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/unstaged-change.txt'],
      UI_TIMEOUT_MS
    );
    need(!!opened, `history/unstaged-change.txt 的 diff 分頁出現並成為目前分頁；目前 DOM：${await contractDump(ctx)}`);

    const rowsReady = await ctx.cdp.poll(
      (p) => {
        const panel = window.__gc.diffPanel(p, 'INDEX', 'WORKTREE');
        return window.__gc.diffRows(panel).length > 0;
      },
      ['history/unstaged-change.txt'],
      UI_TIMEOUT_MS
    );
    need(!!rowsReady, 'diff 內容載入完成（至少一列）');

    const rows = await ctx.cdp.run((p) => window.__gc.diffRows(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')), 'history/unstaged-change.txt');
    check(rows.length === 1 && rows[0].kind === 'row', `history/unstaged-change.txt 只有 1 行、diff 應該只有 1 列 change（實際 ${JSON.stringify(rows)}）`);
    const r0 = rows[0];
    check(
      r0.leftLine === '1' && r0.rightLine === '1' && r0.leftLine === r0.rightLine,
      `同一列左右行號相同、皆為 1（實際 left=${r0.leftLine} right=${r0.rightLine}）`
    );
    check(r0.leftDel && r0.rightAdd, `左側為刪除底色（.diff-row-del）、右側為新增底色（.diff-row-add）（實際 ${JSON.stringify(r0)}）`);
    check(r0.leftText.includes('尚未修改') && r0.rightText.includes('未暫存修改'), `左側為舊內容、右側為新內容（實際 left="${r0.leftText}" right="${r0.rightText}"）`);

    const pathText = await ctx.cdp.run((p) => window.__gc.diffPathText(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')), 'history/unstaged-change.txt');
    check(pathText === 'history/unstaged-change.txt', `工具列路徑文字為完整相對路徑（實際「${pathText}」）`);
    const versionsText = await ctx.cdp.run((p) => window.__gc.diffVersionsText(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')), 'history/unstaged-change.txt');
    check(versionsText === '已暫存 → 工作區', `工具列兩側版本文字為「已暫存 → 工作區」（實際「${versionsText}」）`);

    // --- 2. 工具列四個動作 ---
    const vscodeReady = await ctx.cdp.poll(
      (p) => {
        const panel = window.__gc.diffPanel(p, 'INDEX', 'WORKTREE');
        const btn = window.__gc.diffToolbarButton(panel, '開啟檔案');
        return !!btn && btn.disabled === false;
      },
      ['history/unstaged-change.txt'],
      UI_TIMEOUT_MS
    );
    need(!!vscodeReady, '「開啟檔案」按鈕在工作區檔案存在時變成可按（等中繼資料查詢完成）');
    const vscodeInfo = await ctx.cdp.run((p) => {
      const panel = window.__gc.diffPanel(p, 'INDEX', 'WORKTREE');
      const a = window.__gc.diffToolbarButton(panel, '在 VS Code 開啟');
      return a ? { hidden: a.hidden, href: a.getAttribute('href') } : null;
    }, 'history/unstaged-change.txt');
    check(!!vscodeInfo && vscodeInfo.hidden === false && typeof vscodeInfo.href === 'string' && vscodeInfo.href.startsWith('vscode://'), `「在 VS Code 開啟」顯示且 href 以 vscode:// 開頭（實際 ${JSON.stringify(vscodeInfo)}）`);

    need(
      await ctx.cdp.clickEl((p) => window.__gc.diffToolbarButton(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE'), '看左側版本'), ['history/unstaged-change.txt'], '「看左側版本」按鈕'),
      '點「看左側版本」按鈕'
    );
    const revOpened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.revTab(p, 'INDEX');
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/unstaged-change.txt'],
      UI_TIMEOUT_MS
    );
    check(!!revOpened, `「看左側版本」開啟並選定 rev 分頁（version=INDEX、path=history/unstaged-change.txt）；目前 DOM：${revOpened ? '' : await contractDump(ctx)}`);

    await reselectDiffTab(ctx, 'history/unstaged-change.txt', 'INDEX', 'WORKTREE');
    need(
      await ctx.cdp.clickEl((p) => window.__gc.diffToolbarButton(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE'), '看右側版本'), ['history/unstaged-change.txt'], '「看右側版本」按鈕'),
      '點「看右側版本」按鈕'
    );
    const rightFileOpened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.fileTab(p);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/unstaged-change.txt'],
      UI_TIMEOUT_MS
    );
    check(!!rightFileOpened, '「看右側版本」（to=WORKTREE）開啟並選定檔案分頁');

    const tabCountBefore = await ctx.cdp.run(() => window.__gc.reviewTabs().length);
    await reselectDiffTab(ctx, 'history/unstaged-change.txt', 'INDEX', 'WORKTREE');
    need(
      await ctx.cdp.clickEl((p) => window.__gc.diffToolbarButton(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE'), '開啟檔案'), ['history/unstaged-change.txt'], '「開啟檔案」按鈕'),
      '點「開啟檔案」按鈕'
    );
    const openFileSelected = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.fileTab(p);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/unstaged-change.txt'],
      UI_TIMEOUT_MS
    );
    const tabCountAfter = await ctx.cdp.run(() => window.__gc.reviewTabs().length);
    check(!!openFileSelected && tabCountAfter === tabCountBefore, `「開啟檔案」切到既有檔案分頁、不新增分頁（開啟前 ${tabCountBefore} 個、之後 ${tabCountAfter} 個）`);

    // --- 3. delete 列（deleted-in-worktree.txt：工作區已刪除，INDEX→WORKTREE 整份內容都是 delete）---
    await switchLeftTab(ctx, '變更');
    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['history/deleted-in-worktree.txt'], '「變更」清單的 history/deleted-in-worktree.txt 列'),
      '點「變更」清單的 history/deleted-in-worktree.txt 列'
    );
    const deletedOpened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.diffTab(p, 'INDEX', 'WORKTREE');
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/deleted-in-worktree.txt'],
      UI_TIMEOUT_MS
    );
    need(!!deletedOpened, 'history/deleted-in-worktree.txt 的 diff 分頁出現並成為目前分頁');
    const deletedRowsReady = await ctx.cdp.poll(
      (p) => window.__gc.diffRows(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')).length > 0,
      ['history/deleted-in-worktree.txt'],
      UI_TIMEOUT_MS
    );
    need(!!deletedRowsReady, 'deleted-in-worktree.txt 的 diff 內容載入完成');
    const deletedRows = await ctx.cdp.run((p) => window.__gc.diffRows(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')), 'history/deleted-in-worktree.txt');
    check(
      deletedRows.length > 0 && deletedRows.every((r) => r.kind === 'row' && r.leftDel && r.rightBlank && r.rightLine === '' && r.rightText === ''),
      `整份內容都是 delete 列、右側空白（較暗底色）（實際 ${JSON.stringify(deletedRows)}）`
    );
    const deletedOpenFileDisabled = await ctx.cdp.poll(
      (p) => {
        const btn = window.__gc.diffToolbarButton(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE'), '開啟檔案');
        return !!btn && btn.disabled === true;
      },
      ['history/deleted-in-worktree.txt'],
      UI_TIMEOUT_MS
    );
    check(!!deletedOpenFileDisabled, '工作區已無此檔案時「開啟檔案」停用');
    const deletedVscodeHidden = await ctx.cdp.run((p) => {
      const a = window.__gc.diffToolbarButton(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE'), '在 VS Code 開啟');
      return a ? a.hidden : null;
    }, 'history/deleted-in-worktree.txt');
    check(deletedVscodeHidden === true, '工作區已無此檔案時「在 VS Code 開啟」不顯示');
    const deletedRightBtnState = await ctx.cdp.run((p) => {
      const btn = window.__gc.diffToolbarButton(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE'), '看右側版本');
      return btn ? { hidden: btn.hidden, disabled: btn.disabled } : null;
    }, 'history/deleted-in-worktree.txt');
    // 目視驗收缺陷 V2、Ruling R12：右側為 WORKTREE 時「看右側版本」等同「開啟檔案」（開的是同一個
    // 目前工作區版本的檔案分頁），工作區沒有這個檔案時同樣要停用——仍然顯示（不像「在 VS Code
    // 開啟」整個隱藏），只是不可點。停用樣式的可辨識性另有專門的段落
    // `git-review/刪除檔的工具列停用` 驗證。
    check(
      !!deletedRightBtnState && deletedRightBtnState.hidden === false && deletedRightBtnState.disabled === true,
      `右側為 WORKTREE 時「看右側版本」在工作區沒有這個檔案時仍顯示但停用（實際 ${JSON.stringify(deletedRightBtnState)}）`
    );

    // --- 4. add 列＋多個變更區塊間的 gap 列（docs/design.md 兩處分散插入，中間留足夠未變更的行）---
    // 注意（cockpit-git `gaps_and_hunks_to_rows()` 既有行為，見 file_diff.rs
    // `spec_left_right_pairing_scenario` 測試的註解「最後一個 hunk 之後不插入 gap（檔案共 40 行，
    // 但 patch 不含總行數）」）：unified diff 的 hunk 標頭不帶檔案總行數，所以「最後一個 hunk 之後」
    // 永遠不會有 gap；只有「第一個 hunk 不是從第 1 行開始」（leading）與「兩個 hunk 之間」
    // （between）兩種 gap 保證會出現。這裡特意分兩處插入（間隔夠遠、都不緊貼檔案開頭），驗的正是
    // 「兩個變更區塊之間的 gap」，不主張／不驗證尾端 gap。
    const original = fs.readFileSync(path.join(FIXTURE_SRC, 'docs', 'design.md'), 'utf8');
    const lines = original.split('\n');
    need(lines.length >= 40, `前置：docs/design.md 至少 40 行才夠留出 gap（實際 ${lines.length} 行）`);
    const secondAt = Math.floor(lines.length * 0.75);
    const firstAt = Math.floor(lines.length * 0.25);
    lines.splice(secondAt, 0, 'git-check：左右並排呈現段插入的第二行（供驗兩個區塊之間的 gap 列）');
    lines.splice(firstAt, 0, 'git-check：左右並排呈現段插入的第一行（供驗 add 列）');
    writeTemp(ctx.preview, 'docs/design.md', lines.join('\n'));

    await switchLeftTab(ctx, '變更');
    await waitChangesRow(ctx, 'docs/design.md');
    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['docs/design.md'], '「變更」清單的 docs/design.md 列'),
      '點「變更」清單的 docs/design.md 列'
    );
    const designOpened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.diffTab(p, 'INDEX', 'WORKTREE');
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['docs/design.md'],
      UI_TIMEOUT_MS
    );
    need(!!designOpened, 'docs/design.md 的 diff 分頁出現並成為目前分頁');
    const designRowsReady = await ctx.cdp.poll((p) => window.__gc.diffRows(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')).length > 0, ['docs/design.md'], UI_TIMEOUT_MS);
    need(!!designRowsReady, 'docs/design.md 的 diff 內容載入完成');
    const designRows = await ctx.cdp.run((p) => window.__gc.diffRows(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')), 'docs/design.md');
    // 「兩個變更區塊之間有 gap」＝存在一列 gap，前後都還有別的列（不是整份輸出的第一列或最後一列）。
    const betweenGap = designRows.some((r, i) => r.kind === 'gap' && i > 0 && i < designRows.length - 1);
    check(betweenGap, `兩個分散的變更區塊之間應該有一列 gap（實際 ${JSON.stringify(designRows)}）`);
    const addRows = designRows.filter((r) => r.kind === 'row' && r.rightAdd);
    check(addRows.length === 2, `兩處插入各是一列 add（實際 ${addRows.length} 列；${JSON.stringify(addRows)}）`);
    check(
      addRows.every((r) => r.leftBlank && r.leftLine === '' && r.leftText === ''),
      `add 列左側為空白（較暗底色）（實際 ${JSON.stringify(addRows)}）`
    );
    check(
      addRows.some((r) => r.rightText.includes('第一行')) && addRows.some((r) => r.rightText.includes('第二行')),
      `兩處插入的內容都出現在對應的 add 列（實際 ${JSON.stringify(addRows.map((r) => r.rightText))}）`
    );

    // --- 5. 格線本身是 4 欄 CSS grid ---
    const gridInfo = await ctx.cdp.run((p) => {
      const panel = window.__gc.diffPanel(p, 'INDEX', 'WORKTREE');
      const g = window.__gc.diffGrid(panel);
      if (!g) return null;
      const cs = getComputedStyle(g);
      return { display: cs.display, columns: cs.gridTemplateColumns.trim().split(/\s+/).length };
    }, 'docs/design.md');
    check(!!gridInfo && gridInfo.display === 'grid' && gridInfo.columns === 4, `格線是 4 欄的 CSS grid（實際 ${JSON.stringify(gridInfo)}）`);
  });
}

// 目視驗收缺陷 V1、Ruling R11：GIVEN other-repo（wJ:p5）有一個進行中且有衝突的 merge（design D10
// fixture：conflict.txt，`branch-b` 上合併 `branch-a` 失敗）WHEN 選 wJ:p5、切到「變更」分頁、點
// 「合併衝突」組的 conflict.txt THEN 開出的 diff 分頁是 HEAD（短 hash）→ WORKTREE（不是
// INDEX→WORKTREE——那個組合對未合併檔案會得到 `diff --cc` 三方格式，回 409 `unmerged_path`，見
// `cockpit/src/git.rs` `diff_scenario_merge_conflict_index_to_worktree_is_unmerged_path`），內容
//正常顯示、含衝突標記的新增列、沒有錯誤訊息。
async function segMergeConflictOpenDiff() {
  await withCockpit('conflict-diff', {}, async (ctx) => {
    await openChanges(ctx, PANE_OTHER);
    const root = await rootInfo(ctx, PANE_OTHER);
    check(root.name === 'other-repo' && root.is_git === true, `${PANE_OTHER} 的根目錄＝other-repo、is_git 為 true（實際 ${JSON.stringify(root)}）`);

    await waitChangesRow(ctx, 'conflict.txt');
    const group = await ctx.cdp.run((p) => window.__gc.changesGroupOf(p), 'conflict.txt');
    check(typeof group === 'string' && group.indexOf('合併衝突') === 0, `conflict.txt 在「合併衝突」組（實際「${group}」）`);

    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['conflict.txt'], '「變更」清單的 conflict.txt 列'),
      '點「變更」清單的 conflict.txt 列'
    );
    const opened = await ctx.cdp.poll(
      () => {
        const t = window.__gc.selectedReviewTab();
        return !!t && t.getAttribute('data-diff-path') === 'conflict.txt';
      },
      [],
      UI_TIMEOUT_MS
    );
    need(!!opened, `conflict.txt 的 diff 分頁出現並成為目前分頁；目前 DOM：${await contractDump(ctx)}`);

    const sides = await ctx.cdp.run(() => {
      const t = window.__gc.selectedReviewTab();
      return t ? { from: t.getAttribute('data-diff-from'), to: t.getAttribute('data-diff-to') } : null;
    });
    check(
      !!sides && sides.to === 'WORKTREE' && /^[0-9a-f]{40}$/.test(sides.from || ''),
      `Ruling R11：開出的兩側是 HEAD（40 碼 hash）→ WORKTREE，不是 INDEX→WORKTREE（實際 ${JSON.stringify(sides)}）`
    );
    if (!sides || sides.to !== 'WORKTREE') return; // 前提不成立，後面的斷言沒有意義

    const rowsReady = await ctx.cdp.poll((p, from, to) => window.__gc.diffRows(window.__gc.diffPanel(p, from, to)).length > 0, ['conflict.txt', sides.from, sides.to], UI_TIMEOUT_MS);
    need(!!rowsReady, 'conflict.txt 的 diff 內容載入完成');

    const statusText = await ctx.cdp.run((p, from, to) => window.__gc.diffStatusText(window.__gc.diffPanel(p, from, to)), 'conflict.txt', sides.from, sides.to);
    check(statusText === null, `diff 分頁沒有錯誤訊息（實際狀態文字 ${JSON.stringify(statusText)}）`);

    const rows = await ctx.cdp.run((p, from, to) => window.__gc.diffRows(window.__gc.diffPanel(p, from, to)), 'conflict.txt', sides.from, sides.to);
    const addTexts = rows.filter((r) => r.kind === 'row' && r.rightAdd).map((r) => r.rightText);
    check(
      addTexts.some((t) => t.indexOf('<<<<<<<') === 0) && addTexts.indexOf('=======') !== -1 && addTexts.some((t) => t.indexOf('>>>>>>>') === 0),
      `diff 內容含衝突標記的新增列（實際 ${JSON.stringify(addTexts)}）`
    );
  });
}

// 目視驗收缺陷 V2、Ruling R12：GIVEN review-repo（wJ:p4）的 history/deleted-in-worktree.txt 已在
// 工作區刪除（task 3.1 fixture）WHEN 開啟它的 diff 分頁（暫存區 → 工作區）THEN 「開啟檔案」與
// 「看右側版本」（右側為 WORKTREE）都停用（`disabled` 屬性），且計算樣式（`color`）與同一顆按鈕在
// 可用狀態下不同——單靠 `disabled` 屬性不夠，spec 要求「可辨識的停用樣式」（見
// `style.css` `.action-button:disabled`：用 `color-mix` 從 `--text-dim` 推導出更淡的版本，因為
// 啟用狀態的 `.action-button` 靜止時本來就是 `--text-dim`，兩者在計算樣式上要能分得開）。
async function segDeletedFileToolbarDisabled() {
  await withCockpit('deleted-toolbar', {}, async (ctx) => {
    await openChanges(ctx, PANE_REVIEW);
    await waitChangesRow(ctx, 'history/deleted-in-worktree.txt');
    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['history/deleted-in-worktree.txt'], '「變更」清單的 history/deleted-in-worktree.txt 列'),
      '點「變更」清單的 history/deleted-in-worktree.txt 列'
    );
    const opened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.diffTab(p, 'INDEX', 'WORKTREE');
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/deleted-in-worktree.txt'],
      UI_TIMEOUT_MS
    );
    need(!!opened, 'history/deleted-in-worktree.txt 的 diff 分頁出現並成為目前分頁');
    await ctx.cdp.poll((p) => window.__gc.diffRows(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')).length > 0, ['history/deleted-in-worktree.txt'], UI_TIMEOUT_MS);
    // 等 refreshDiffWorkFile() 的 meta 查詢回來、確認工作區沒有這個檔案，按鈕狀態才會穩定。
    await ctx.cdp.poll(
      (p) => {
        const btn = window.__gc.diffToolbarButton(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE'), '開啟檔案');
        return !!btn && btn.disabled === true;
      },
      ['history/deleted-in-worktree.txt'],
      UI_TIMEOUT_MS
    );

    const deletedState = await ctx.cdp.run((p) => {
      const panel = window.__gc.diffPanel(p, 'INDEX', 'WORKTREE');
      const openBtn = window.__gc.diffToolbarButton(panel, '開啟檔案');
      const rightBtn = window.__gc.diffToolbarButton(panel, '看右側版本');
      return {
        openDisabled: openBtn ? openBtn.disabled : null,
        rightHidden: rightBtn ? rightBtn.hidden : null,
        rightDisabled: rightBtn ? rightBtn.disabled : null,
        openColor: openBtn ? getComputedStyle(openBtn).color : null,
        rightColor: rightBtn ? getComputedStyle(rightBtn).color : null,
      };
    }, 'history/deleted-in-worktree.txt');
    check(deletedState.openDisabled === true, `「開啟檔案」disabled 屬性為 true（實際 ${JSON.stringify(deletedState)}）`);
    check(deletedState.rightHidden === false && deletedState.rightDisabled === true, `「看右側版本」不隱藏但 disabled 屬性為 true（實際 ${JSON.stringify(deletedState)}）`);

    // 跟同一個分頁裡「已暫存」組的 history/staged-change.txt（工作區存在，「開啟檔案」可用）的
    // 「開啟檔案」按鈕比較計算樣式，確認停用真的反映在樣式上，不是只有屬性生效。
    await switchLeftTab(ctx, '變更');
    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['history/staged-change.txt'], '「變更」清單的 history/staged-change.txt 列'),
      '點「變更」清單的 history/staged-change.txt 列'
    );
    const stagedOpened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.selectedReviewTab();
        return !!t && t.getAttribute('data-diff-path') === p && t.getAttribute('aria-selected') === 'true';
      },
      ['history/staged-change.txt'],
      UI_TIMEOUT_MS
    );
    need(!!stagedOpened, 'history/staged-change.txt 的 diff 分頁出現並成為目前分頁');
    // 等 refreshDiffWorkFile() 的中繼資料查詢回來（同刪除檔那邊的前置）：按鈕預設是停用的
    // （tab.workFile 初始為 null），要等非同步查詢確認工作區有這個檔案才會變可用。
    await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.selectedReviewTab();
        if (!t || t.getAttribute('data-diff-path') !== p) return false;
        const panel = window.__gc.diffPanel(p, t.getAttribute('data-diff-from'), t.getAttribute('data-diff-to'));
        const btn = window.__gc.diffToolbarButton(panel, '開啟檔案');
        return !!btn && btn.disabled === false;
      },
      ['history/staged-change.txt'],
      UI_TIMEOUT_MS
    );
    const enabledState = await ctx.cdp.run((p) => {
      const t = window.__gc.selectedReviewTab();
      if (!t || t.getAttribute('data-diff-path') !== p) return null;
      const panel = window.__gc.diffPanel(p, t.getAttribute('data-diff-from'), t.getAttribute('data-diff-to'));
      const btn = window.__gc.diffToolbarButton(panel, '開啟檔案');
      return btn ? { disabled: btn.disabled, color: getComputedStyle(btn).color } : null;
    }, 'history/staged-change.txt');
    check(
      !!enabledState && enabledState.disabled === false,
      `前置：history/staged-change.txt 的「開啟檔案」應為可用狀態（工作區存在這個檔案；實際 ${JSON.stringify(enabledState)}）`
    );
    check(
      !!enabledState && !!deletedState.openColor && enabledState.color !== deletedState.openColor,
      `停用按鈕與可用按鈕的計算樣式（color）不同（停用 ${deletedState.openColor}／可用 ${enabledState && enabledState.color}）`
    );
  });
}

// GIVEN long.md 的 diff 分頁（暫存區 → 工作區）為目前分頁，已往下捲動（前置：多處分散的修改讓內容
// 夠高、確實需要捲動）WHEN 在檔案末端新增一行 THEN 3 秒內新行以新增列出現，捲動位置不變。
async function segUpdateAfterChange() {
  await withCockpit('diff-update', {}, async (ctx) => {
    const original = fs.readFileSync(path.join(FIXTURE_SRC, 'long.md'), 'utf8');
    const lines = original.split('\n');
    need(lines.length > 200, `前置：long.md 應該有足夠行數可以分散製造多個 hunk（實際 ${lines.length} 行）`);
    for (let i = 20; i < lines.length - 20; i += 40) {
      lines[i] = `${lines[i]}（git-check 改動 #${i}）`;
    }
    writeTemp(ctx.preview, 'long.md', lines.join('\n'));

    await openChanges(ctx);
    await waitChangesRow(ctx, 'long.md');
    need(await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['long.md'], '「變更」清單的 long.md 列'), '點「變更」清單的 long.md 列');
    const opened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.diffTab(p, 'INDEX', 'WORKTREE');
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['long.md'],
      UI_TIMEOUT_MS
    );
    need(!!opened, 'long.md 的 diff 分頁（暫存區 → 工作區）出現並成為目前分頁');

    const ready = await ctx.cdp.poll(
      (p) => {
        const panel = window.__gc.diffPanel(p, 'INDEX', 'WORKTREE');
        const host = window.__gc.diffHost(panel);
        return !!host && host.scrollHeight > host.clientHeight + 20;
      },
      ['long.md'],
      UI_TIMEOUT_MS
    );
    need(!!ready, `前置：diff 內容夠高、確實需要捲動（多處分散修改製造多個 hunk／gap）；目前 DOM：${await contractDump(ctx)}`);

    const scrollTopSet = await ctx.cdp.run((p) => {
      const host = window.__gc.diffHost(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE'));
      host.scrollTop = Math.round(host.scrollHeight / 2);
      return host.scrollTop;
    }, 'long.md');
    need(scrollTopSet > 0, `前置：diff 內容已往下捲動（scrollTop=${scrollTopSet}）`);
    await sleep(300); // 讓 scroll 事件寫回 tab.scroll，避免輪詢重畫時讀到還沒寫回的舊值

    const appended = lines.slice();
    appended.push('git-check 在檔案末端新增的一行（改檔後更新）');
    const t0 = Date.now();
    writeTemp(ctx.preview, 'long.md', appended.join('\n'));

    const seen = await ctx.cdp.poll(
      (p) => {
        const panel = window.__gc.diffPanel(p, 'INDEX', 'WORKTREE');
        return panel ? window.__gc.txt(panel).indexOf('git-check 在檔案末端新增的一行') !== -1 : false;
      },
      ['long.md'],
      3000
    );
    const elapsed = Date.now() - t0;
    check(!!seen, `3 秒內新行以新增列出現（${seen ? `${elapsed} ms` : '逾時'}）`);

    const scrollTopAfter = await ctx.cdp.run((p) => window.__gc.diffHost(window.__gc.diffPanel(p, 'INDEX', 'WORKTREE')).scrollTop, 'long.md');
    check(Math.abs(scrollTopAfter - scrollTopSet) <= 2, `捲動位置不變（${scrollTopSet} → ${scrollTopAfter}）`);
  });
}

// GIVEN 視窗寬 700，diff 中有 300 個字元的長行（未追蹤檔案，EMPTY → WORKTREE）WHEN 顯示該 diff
// THEN 長行在所屬欄內折行，頁面沒有橫向捲軸。以預設寬度（1536）完成選取 pane、開分頁等互動——
// 700 寬時版面改成單欄＋整頁捲動（spec cockpit-dashboard「窄視窗單欄」），`scrollIntoView` 之後
// pane 列可能被 sticky 底列擋住點不到；只在最後量測時才切到 700 寬（同「顯示變更並開啟 diff」段
// 對 1536／1100 兩個寬度做 bounding rect 檢查的既有手法：互動在預設寬度做，只在需要量測的當下切換
// `Emulation.setDeviceMetricsOverride`）。
async function segNoHorizontalScroll() {
  await withCockpit('no-h-scroll', {}, async (ctx) => {
    const longLine = 'x'.repeat(300);
    writeTemp(ctx.preview, 'history/git-check-longline.md', `${longLine}\n`);

    await openChanges(ctx);
    await waitChangesRow(ctx, 'history/git-check-longline.md');
    const group = await ctx.cdp.run((p) => window.__gc.changesGroupOf(p), 'history/git-check-longline.md');
    check(group === '未追蹤（2）', `git-check-longline.md 在「未追蹤」組（實際「${group}」；原有 untracked-file.md＋新檔＝2）`);

    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['history/git-check-longline.md'], '「變更」清單的 history/git-check-longline.md 列'),
      '點「變更」清單的 history/git-check-longline.md 列'
    );
    const opened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.diffTab(p, 'EMPTY', 'WORKTREE');
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/git-check-longline.md'],
      UI_TIMEOUT_MS
    );
    need(!!opened, 'history/git-check-longline.md 的 diff 分頁（EMPTY → WORKTREE）出現並成為目前分頁');
    const rowsReady = await ctx.cdp.poll(
      (p) => window.__gc.diffRows(window.__gc.diffPanel(p, 'EMPTY', 'WORKTREE')).length > 0,
      ['history/git-check-longline.md'],
      UI_TIMEOUT_MS
    );
    need(!!rowsReady, 'git-check-longline.md 的 diff 內容載入完成');

    await ctx.cdp.send('Emulation.setDeviceMetricsOverride', { width: 700, height: 900, deviceScaleFactor: 1, mobile: false });
    await sleep(300);

    const measured = await ctx.cdp.run((p) => {
      const panel = window.__gc.diffPanel(p, 'EMPTY', 'WORKTREE');
      const grid = window.__gc.diffGrid(panel);
      const cells = grid ? Array.from(grid.querySelectorAll('.diff-text')) : [];
      const cell = cells.find((c) => c.textContent.length >= 300);
      const lineHeight = cell ? parseFloat(getComputedStyle(cell).lineHeight) : null;
      const cellHeight = cell ? cell.getBoundingClientRect().height : null;
      return {
        cellFound: !!cell,
        lineHeight,
        cellHeight,
        wrapped: !!cell && !!lineHeight && cellHeight > lineHeight * 1.5,
        docScrollWidth: document.documentElement.scrollWidth,
        docClientWidth: document.documentElement.clientWidth,
      };
    }, 'history/git-check-longline.md');
    check(measured.cellFound, `找到含 300 字元長行的 .diff-text 儲存格（實際 ${JSON.stringify(measured)}）`);
    check(measured.wrapped, `長行在所屬欄內折行（渲染高度 ${measured.cellHeight} > 1.5 倍行高 ${measured.lineHeight}）`);
    check(measured.docScrollWidth <= measured.docClientWidth + 1, `頁面沒有橫向捲軸（documentElement scrollWidth ${measured.docScrollWidth} ≤ clientWidth ${measured.docClientWidth}）`);
  });
}

// ---------------------------------------------------------------------------
// git-review：Git Graph 分頁（git-review task 4.4；spec「Git Graph 分頁」；design D8／D9）
// ---------------------------------------------------------------------------

// 開啟該 repo 的 Git Graph 分頁並等它成為目前分頁（同 segShowChangesAndOpenDiff 的「額外驗證」
// 那一段，抽成共用函式給下面四段都用）。
async function openGraphTab(ctx, root) {
  const btnReady = await ctx.cdp.poll(() => {
    const b = window.__gc.changesButton('Git Graph');
    return !!b && window.__gc.visible(b);
  }, [], UI_TIMEOUT_MS);
  need(!!btnReady, `前置：「Git Graph」按鈕出現且可見；目前 DOM：${await contractDump(ctx)}`);
  need(await ctx.cdp.clickEl(() => window.__gc.changesButton('Git Graph'), [], '「Git Graph」按鈕'), '點「Git Graph」按鈕');
  const opened = await ctx.cdp.poll(
    (rid) => {
      const t = window.__gc.graphTab(rid);
      return !!t && t.getAttribute('aria-selected') === 'true';
    },
    [root.root_id],
    UI_TIMEOUT_MS
  );
  if (!opened) {
    need(false, `按「Git Graph」後應該出現並選定該 repo 的 Git Graph 分頁；目前 DOM：${await contractDump(ctx)}`);
  }
}

// GIVEN repo 有 264 個 commit（task 3.1 fixture：263 個 fast-import commit＋根 commit）WHEN 在
// 「變更」分頁按「Git Graph」，再捲到清單底部 THEN 先顯示 200 列，捲到底部後共顯示 264 列，沒有
// 重複的 commit，末端沒有「載入更多」。額外驗（自我驗證，非 spec 逐字要求）：分支篩選 popover
// 列出三組（本地分支／遠端分支／tag）且可用 Esc 關閉並把焦點還給按鈕；HEAD 節點與三種 ref 標籤
// （本地分支／遠端分支／tag）都出現在已載入範圍內。
async function segOpenAndBatchLoad() {
  await withCockpit('open-batch-load', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await openChanges(ctx);
    await openGraphTab(ctx, root);

    const firstBatch = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
    need(!!firstBatch, `第一批應該先顯示 200 列；目前 DOM：${await contractDump(ctx)}`);
    const firstCount = await ctx.cdp.run((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length, root.root_id);
    check(firstCount === 200, `第一批恰好 200 列（實際 ${firstCount}）`);

    const loadMoreVisible = await ctx.cdp.run((rid) => {
      const p = window.__gc.graphPanelOf(rid);
      const btn = window.__gc.graphButtonByAction(p, 'graph-load-more');
      return !!btn && !btn.hidden;
    }, root.root_id);
    check(loadMoreVisible, '第一批之後「載入更多」按鈕可見（has_more 為 true）');

    // 捲到清單底部（同「載入更多」按鈕的鍵盤可達替代路徑：直接捲動觸發自動載入）；反覆捲到底再
    // 等一輪讀取，直到「載入更多」按鈕消失或逾時。
    let finalRows = firstCount;
    let loadMoreGone = false;
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
      await ctx.cdp.run((rid) => {
        const sc = window.__gc.graphScroller(window.__gc.graphPanelOf(rid));
        if (sc) sc.scrollTop = sc.scrollHeight;
      }, root.root_id);
      await sleep(400);
      const state = await ctx.cdp.run((rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const btn = window.__gc.graphButtonByAction(p, 'graph-load-more');
        return { rows: window.__gc.graphRows(p).length, hidden: !btn || btn.hidden };
      }, root.root_id);
      finalRows = state.rows;
      loadMoreGone = state.hidden;
      if (loadMoreGone) break;
    }
    check(loadMoreGone, `捲到底部後「載入更多」按鈕消失（逾時前最後狀態：rows=${finalRows}）`);
    check(finalRows === 264, `捲到底部後共顯示 264 列（實際 ${finalRows}）`);

    const dupCheck = await ctx.cdp.run((rid) => {
      const oids = window.__gc.graphRows(window.__gc.graphPanelOf(rid)).map((r) => r.getAttribute('data-oid'));
      return { total: oids.length, uniq: new Set(oids).size };
    }, root.root_id);
    check(dupCheck.total === dupCheck.uniq, `沒有重複的 commit（${dupCheck.total} 列，${dupCheck.uniq} 個不重複 oid）`);

    const capNoteVisible = await ctx.cdp.run((rid) => {
      const p = window.__gc.graphPanelOf(rid);
      return Array.from(p.querySelectorAll('.tree-note')).some((n) => window.__gc.visible(n) && window.__gc.txt(n).includes('已達上限'));
    }, root.root_id);
    check(!capNoteVisible, '264 筆未達 5000 上限，不顯示「已達上限」提示（DOM 節點本來就存在、只是 hidden，這裡驗的是可見性）');

    // 額外驗：HEAD 節點與三種 ref 標籤都出現在已載入範圍內（design D8：HEAD 節點「實心加外框」；
    // spec：本地分支／遠端分支／tag 各有可區分的樣式）。
    const badgeKinds = await ctx.cdp.run((rid) => {
      const p = window.__gc.graphPanelOf(rid);
      const kinds = new Set(Array.from(p.querySelectorAll('.graph-ref-badge')).map((b) => b.getAttribute('data-kind')));
      return { kinds: Array.from(kinds).sort(), headRing: p.querySelectorAll('.graph-node-head-ring').length, headBranchBadge: p.querySelectorAll('.graph-ref-badge.is-head-branch').length };
    }, root.root_id);
    check(JSON.stringify(badgeKinds.kinds) === JSON.stringify(['branch', 'remote', 'tag']), `三種 ref 標籤都出現（實際 ${JSON.stringify(badgeKinds.kinds)}）`);
    check(badgeKinds.headRing === 1, `HEAD 節點的外框只出現一次（實際 ${badgeKinds.headRing}）`);
    check(badgeKinds.headBranchBadge === 1, `HEAD 指向的分支標籤只出現一次（實際 ${badgeKinds.headBranchBadge}）`);

    // 額外驗：分支篩選 popover 列出三組、Esc 關閉並把焦點還給按鈕。
    need(
      await ctx.cdp.clickEl((rid) => window.__gc.graphButtonByAction(window.__gc.graphPanelOf(rid), 'graph-filter-toggle'), [root.root_id], '「分支篩選」按鈕'),
      '點「分支篩選」按鈕'
    );
    const popoverOpen = await ctx.cdp.poll(
      (rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const pop = p.querySelector('.graph-filter-popover');
        return !!pop && !pop.hidden;
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!popoverOpen, '分支篩選 popover 打開');
    const groups = await ctx.cdp.run((rid) => {
      const p = window.__gc.graphPanelOf(rid);
      return Array.from(p.querySelectorAll('.graph-filter-group-title')).map((t) => window.__gc.txt(t));
    }, root.root_id);
    check(JSON.stringify(groups) === JSON.stringify(['本地分支', '遠端分支', 'tag']), `popover 依本地分支／遠端分支／tag 分組（實際 ${JSON.stringify(groups)}）`);

    await ctx.cdp.run(() => {
      document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      return true;
    });
    const closedAndFocused = await ctx.cdp.poll(
      (rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const pop = p.querySelector('.graph-filter-popover');
        const toggle = window.__gc.graphButtonByAction(p, 'graph-filter-toggle');
        return !!pop && pop.hidden && document.activeElement === toggle;
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    check(!!closedAndFocused, 'Esc 關閉 popover 且焦點回到「分支篩選」按鈕');
  });
}

// GIVEN Git Graph 已載入，有 3 個訊息含「git-check-search-hit」的 commit（大小寫混合，見下方
// addChainedEmptyCommits；spec scenario 原文用「fix」，但 review-repo 的合成歷史每筆 subject
// 都帶有「fixture」四個字，逐字比對「fix」會命中全部 264 筆而不是 3 筆——見 self 審查，這裡換一個
// fixture 裡不會自然出現的字串，驗的是同一條 spec 規則）WHEN 在搜尋框輸入全大寫的
// 「GIT-CHECK-SEARCH-HIT」並按兩次 Enter THEN 顯示「第 2／共 3 筆」，第二個相符的列在可見範圍內
// 且被標示；清單列數不變。
async function segSearchJump() {
  await withCockpit(
    'search-jump',
    {
      beforeLoad: (preview) => {
        addChainedEmptyCommits(preview.reviewRepo, 'git-check-search', ['git-check-search-hit case A', 'git-check-search-hit case B', 'git-check-search-hit case C']);
      },
    },
    async (ctx) => {
      const root = await rootInfo(ctx, PANE_REVIEW);
      await openChanges(ctx);
      await openGraphTab(ctx, root);
      const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
      need(!!loaded, '第一批 200 列載入完成');

      const rowsBefore = await ctx.cdp.run((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length, root.root_id);
      const preCount = await ctx.cdp.run((rid) => {
        const p = window.__gc.graphPanelOf(rid);
        return window.__gc.graphRows(p).filter((r) => /git-check-search-hit/i.test(window.__gc.txt(r.querySelector('.graph-subject')))).length;
      }, root.root_id);
      need(preCount === 3, `前置：3 筆含 git-check-search-hit 的 commit 都在已載入範圍內（實際 ${preCount}）`);

      need(await ctx.cdp.clickEl((rid) => window.__gc.graphSearchInput(window.__gc.graphPanelOf(rid)), [root.root_id], '搜尋框'), '點搜尋框');
      await ctx.cdp.run((rid) => {
        const input = window.__gc.graphSearchInput(window.__gc.graphPanelOf(rid));
        input.value = 'GIT-CHECK-SEARCH-HIT';
        input.dispatchEvent(new Event('input', { bubbles: true }));
        return true;
      }, root.root_id);
      const countAfterType = await ctx.cdp.poll((rid) => window.__gc.graphSearchCount(window.__gc.graphPanelOf(rid)) === '共 3 筆', [root.root_id], UI_TIMEOUT_MS);
      need(!!countAfterType, `輸入後（尚未按 Enter）先只顯示總筆數「共 3 筆」（不分大小寫比對，輸入全大寫仍命中）`);

      async function pressEnter() {
        await ctx.cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter' });
        await ctx.cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter' });
      }
      await pressEnter();
      const after1 = await ctx.cdp.poll((rid) => window.__gc.graphSearchCount(window.__gc.graphPanelOf(rid)) === '第 1／共 3 筆', [root.root_id], UI_TIMEOUT_MS);
      need(!!after1, '第一次 Enter 顯示「第 1／共 3 筆」');
      await pressEnter();
      const after2 = await ctx.cdp.poll((rid) => window.__gc.graphSearchCount(window.__gc.graphPanelOf(rid)) === '第 2／共 3 筆', [root.root_id], UI_TIMEOUT_MS);
      check(!!after2, `第二次 Enter 顯示「第 2／共 3 筆」（實際 ${await ctx.cdp.run((rid) => window.__gc.graphSearchCount(window.__gc.graphPanelOf(rid)), root.root_id)}）`);

      const hit = await ctx.cdp.run((rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const el = p.querySelector('.is-search-hit');
        if (!el) return null;
        const r = el.getBoundingClientRect();
        const host = window.__gc.graphScroller(p).getBoundingClientRect();
        return { subject: window.__gc.txt(el.querySelector('.graph-subject')), inView: r.top >= host.top - 1 && r.bottom <= host.bottom + 1, count: p.querySelectorAll('.is-search-hit').length };
      }, root.root_id);
      check(!!hit && hit.count === 1, `恰好一列被標示為目前命中（實際 ${JSON.stringify(hit)}）`);
      check(!!hit && hit.subject === 'git-check-search-hit case B', `第二個相符的列是第二筆 commit（實際「${hit && hit.subject}」）`);
      check(!!hit && hit.inView, `被標示的列在捲動容器可見範圍內（實際 ${JSON.stringify(hit)}）`);

      const rowsAfter = await ctx.cdp.run((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length, root.root_id);
      check(rowsAfter === rowsBefore, `搜尋不改變清單列數（${rowsBefore} → ${rowsAfter}）`);
    }
  );
}

// ui-fixes task 4.5（spec「搜尋命中後背景載入不拉動捲動」）GIVEN Git Graph 已載入前 200 列，搜尋後按
// Enter 跳到位於清單前段的第一筆命中，接著往下捲到清單底部 WHEN 因捲到底部而載入下一批 THEN 載入完成後
// `scrollTop` 與載入前相同、「第 i／共 n 筆」的 i 不變、n 反映新的命中數；再按 Enter 後下一筆命中列才被
// 捲入可見範圍並標示。
// fixture：疊在 HEAD 上開新分支（addChainedEmptyCommits），由上而下＝40 個無關 commit、命中 X（第 41 列）、
// 169 個無關 commit、命中 Y（第 211 列，落在第二批）；X 與 Y 相距很遠，Enter 才看得出「捲到下一筆」。
async function segSearchBackgroundLoadNoScroll() {
  const HIT = 'git-check-scroll-hit';
  await withCockpit(
    'search-bg-load',
    {
      beforeLoad: (preview) => {
        const subjects = [`${HIT} Y`];
        for (let i = 0; i < 169; i += 1) subjects.push(`git-check-plain-lower ${i}`);
        subjects.push(`${HIT} X`);
        for (let i = 0; i < 40; i += 1) subjects.push(`git-check-plain-upper ${i}`);
        addChainedEmptyCommits(preview.reviewRepo, 'git-check-scroll', subjects);
      },
    },
    async (ctx) => {
      const root = await rootInfo(ctx, PANE_REVIEW);
      await openChanges(ctx);
      await openGraphTab(ctx, root);
      const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
      need(!!loaded, '第一批 200 列載入完成');
      const rows0 = await ctx.cdp.run((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length, root.root_id);
      need(rows0 === 200, `前置：此時恰好 200 列（尚未載入第二批；實際 ${rows0}）`);

      need(await ctx.cdp.clickEl((rid) => window.__gc.graphSearchInput(window.__gc.graphPanelOf(rid)), [root.root_id], '搜尋框'), '點搜尋框');
      await ctx.cdp.run((rid, q) => {
        const input = window.__gc.graphSearchInput(window.__gc.graphPanelOf(rid));
        input.value = q;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        return true;
      }, root.root_id, HIT);
      const typed = await ctx.cdp.poll((rid) => window.__gc.graphSearchCount(window.__gc.graphPanelOf(rid)) === '共 1 筆', [root.root_id], UI_TIMEOUT_MS);
      need(!!typed, '輸入後顯示「共 1 筆」（第二批的命中 Y 尚未載入）');

      async function pressEnter() {
        await ctx.cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter' });
        await ctx.cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter' });
      }
      await pressEnter();
      const jumped = await ctx.cdp.poll((rid) => window.__gc.graphSearchCount(window.__gc.graphPanelOf(rid)) === '第 1／共 1 筆', [root.root_id], UI_TIMEOUT_MS);
      need(!!jumped, 'Enter 後顯示「第 1／共 1 筆」');
      const hitX = await ctx.cdp.run((rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const el = p.querySelector('.is-search-hit');
        const sc = window.__gc.graphScroller(p);
        return { subject: el ? window.__gc.txt(el.querySelector('.graph-subject')) : null, scrollTop: sc.scrollTop };
      }, root.root_id);
      need(hitX.subject === `${HIT} X` && hitX.scrollTop > 0, `前置：Enter 跳到前段命中 X 且清單已捲離頂端（實際 ${JSON.stringify(hitX)}）`);

      // 捲到清單底部，離開命中列並觸發下一批載入；記下載入前的 scrollTop。
      const before = await ctx.cdp.run((rid) => {
        const sc = window.__gc.graphScroller(window.__gc.graphPanelOf(rid));
        sc.scrollTop = sc.scrollHeight;
        return sc.scrollTop;
      }, root.root_id);
      const batch2 = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length > 200, [root.root_id], UI_TIMEOUT_MS);
      need(!!batch2, '捲到底部後載入了下一批（列數超過 200）');
      const countUpdated = await ctx.cdp.poll((rid) => /共 2 筆/.test(window.__gc.graphSearchCount(window.__gc.graphPanelOf(rid))), [root.root_id], UI_TIMEOUT_MS);
      check(!!countUpdated, 'n 反映新的命中數（共 2 筆）');
      await sleep(500); // 讓載入後的重算與可能的 scrollIntoView 都跑完
      const after = await ctx.cdp.run((rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const sc = window.__gc.graphScroller(p);
        return { scrollTop: sc.scrollTop, count: window.__gc.graphSearchCount(p), rows: window.__gc.graphRows(p).length };
      }, root.root_id);
      check(Math.abs(after.scrollTop - before) <= 1, `載入完成後 scrollTop 與載入前相同（${before} → ${after.scrollTop}）`);
      check(after.count === '第 1／共 2 筆', `「第 i／共 n 筆」的 i 不變、n 更新（實際「${after.count}」）`);

      // 再按 Enter：下一筆命中 Y 才被捲入可見範圍並標示。
      await pressEnter();
      const next = await ctx.cdp.poll((rid) => window.__gc.graphSearchCount(window.__gc.graphPanelOf(rid)) === '第 2／共 2 筆', [root.root_id], UI_TIMEOUT_MS);
      check(!!next, '再按 Enter 顯示「第 2／共 2 筆」');
      const hitY = await ctx.cdp.run((rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const el = p.querySelector('.is-search-hit');
        if (!el) return null;
        const r = el.getBoundingClientRect();
        const host = window.__gc.graphScroller(p).getBoundingClientRect();
        return { subject: window.__gc.txt(el.querySelector('.graph-subject')), inView: r.top >= host.top - 1 && r.bottom <= host.bottom + 1, count: p.querySelectorAll('.is-search-hit').length };
      }, root.root_id);
      check(!!hitY && hitY.count === 1 && hitY.subject === `${HIT} Y`, `恰好一列被標示、且是第二筆命中 Y（實際 ${JSON.stringify(hitY)}）`);
      check(!!hitY && hitY.inView, `按 Enter 後命中列 Y 被捲入可見範圍（實際 ${JSON.stringify(hitY)}）`);
    }
  );
}

// ui-fixes task 4.6（spec「有非 commit tag 時不誤報分支變更」）GIVEN repo 有一個指向 tree 物件的 tag，
// Git Graph 分頁為目前分頁，repo 沒有任何變動 WHEN 經過 5 秒 THEN 不出現「分支已變更」。
// 這個 tree tag 只加在本段專用的 ui_preview 暫存副本（每個 withCockpit 各自一份），不牽動其他段的 refs
// 計數與篩選選單項目數。額外驗：同樣有 tree tag 時，真的新增 commit 仍然 3 秒內出現「分支已變更」
// （偵測沒有因此失效）。
async function segNonCommitTagNoFalseBanner() {
  await withCockpit(
    'non-commit-tag',
    {
      beforeLoad: (preview) => {
        const tree = gitTemp(preview.reviewRepo, ['rev-parse', 'HEAD^{tree}']);
        gitTemp(preview.reviewRepo, ['tag', 'tree-tag', tree]);
      },
    },
    async (ctx) => {
      const root = await rootInfo(ctx, PANE_REVIEW);
      const refs = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/refs`);
      const treeRef = refs.body && Array.isArray(refs.body.refs) ? refs.body.refs.find((r) => r.short === 'tree-tag') : null;
      need(!!treeRef && treeRef.commit === false, `前置：refs 端點列出 tree-tag 且 commit 為 false（實際 ${JSON.stringify(treeRef)}）`);
      await openChanges(ctx);
      await openGraphTab(ctx, root);
      const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
      need(!!loaded, '第一批 200 列載入完成（log 沒有因 tree tag 失敗）');

      // 5 秒內每 250 ms 看一次，任何一次出現 banner 就失敗（輪詢間隔 2 秒，涵蓋 2 次以上的 refs 讀取）。
      let shownAt = null;
      const t0 = Date.now();
      while (Date.now() - t0 < 5500) {
        const hidden = await ctx.cdp.run((rid) => window.__gc.graphBanner(window.__gc.graphPanelOf(rid)).hidden, root.root_id);
        if (!hidden) {
          shownAt = Date.now() - t0;
          break;
        }
        await sleep(250);
      }
      check(shownAt === null, `有指向 tree 的 tag、repo 沒有變動：5 秒內不出現「分支已變更」（${shownAt === null ? '未出現' : `${shownAt} ms 時出現`}）`);

      gitTemp(ctx.preview.reviewRepo, ['-c', 'user.name=git-check', '-c', 'user.email=git-check@invalid', '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'git-check: tree tag 並存下的分支變更']);
      const shown = await ctx.cdp.poll((rid) => !window.__gc.graphBanner(window.__gc.graphPanelOf(rid)).hidden, [root.root_id], 3000);
      check(!!shown, '有 tree tag 時，新增 commit 後 3 秒內仍會出現「分支已變更」（偵測沒有失效）');
    }
  );
}

// ui-fixes task 4.7（spec「commit 詳情與比較」）共用：頁面端探針，回報 commit 詳情區有沒有「變更過多，
// 只列出前面一部分」提示、提示是否在檔案清單之後（清單末端）、以及清單列數。
function truncationNoteProbe(rid) {
  const w = window.__gc.commitDetailWrap(window.__gc.graphPanelOf(rid));
  if (!w) return null;
  const note = Array.from(w.querySelectorAll('*')).find((n) => n.children.length === 0 && window.__gc.txt(n) === '變更過多，只列出前面一部分') || null;
  const list = w.querySelector('.commit-detail-files');
  return {
    hasNote: !!note,
    afterList: !!note && !!list && !!(list.compareDocumentPosition(note) & Node.DOCUMENT_POSITION_FOLLOWING),
    rows: w.querySelectorAll('.commit-detail-file-row').length,
  };
}

// GIVEN 某 commit 變更的檔案多到後端回報 truncated 為 true WHEN 在 Git Graph 選取它 THEN 詳情的檔案清單
// 末端顯示「變更過多，只列出前面一部分」；檔案數未被截斷的 commit（HEAD）不顯示此提示。
async function segCommitDetailTruncationNote() {
  await withCockpit(
    'commit-detail-truncated',
    {
      beforeLoad: (preview) => {
        preview.huge = addHugeChangeCommit(preview.reviewRepo, 'git-check-huge');
      },
    },
    async (ctx) => {
      const root = await rootInfo(ctx, PANE_REVIEW);
      const huge = ctx.preview.huge;
      const apiBig = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/commit/${huge.oid}`);
      need(
        apiBig.status === 200 && apiBig.body && apiBig.body.truncated === true && Array.isArray(apiBig.body.files) && apiBig.body.files.length < huge.count,
        `前置：後端對大量變更的 commit 回 truncated=true 且檔案數少於實際 ${huge.count}（實際 ${apiBig.status} truncated=${apiBig.body && apiBig.body.truncated} files=${apiBig.body && apiBig.body.files && apiBig.body.files.length}）`
      );
      const status = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/status`);
      need(status.status === 200 && status.body && status.body.branch, `前置：狀態端點回 200（實際 ${status.status}）`);
      const headOid = status.body.branch.oid;
      const apiHead = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/commit/${headOid}`);
      need(apiHead.status === 200 && apiHead.body && apiHead.body.truncated === false, `前置：一般 commit（HEAD）的 truncated 為 false（實際 ${JSON.stringify(apiHead.body && apiHead.body.truncated)}）`);

      await openChanges(ctx);
      await openGraphTab(ctx, root);
      const rowsLoaded = await ctx.cdp.poll((rid, a, b) => !!window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), a) && !!window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), b), [root.root_id, huge.oid, headOid], UI_TIMEOUT_MS);
      need(!!rowsLoaded, '前置：HEAD 列與大量變更 commit 列都已載入');

      async function openDetail(oid, label) {
        need(await ctx.cdp.clickEl((rid, o) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), o), [root.root_id, oid], label), `點${label}`);
        const ready = await ctx.cdp.poll(
          (rid, o) => {
            const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
            return !!t && t.includes(o) && !t.includes('正在讀取');
          },
          [root.root_id, oid],
          UI_TIMEOUT_MS
        );
        need(!!ready, `${label}的詳情載入完成`);
      }

      await openDetail(headOid, 'HEAD（未被截斷）列');
      const normal = await ctx.cdp.run(truncationNoteProbe, root.root_id);
      check(!!normal && normal.rows > 0 && normal.hasNote === false, `未被截斷的 commit：詳情不顯示「變更過多，只列出前面一部分」（實際 ${JSON.stringify(normal)}）`);

      await openDetail(huge.oid, '大量變更（被截斷）commit 列');
      const big = await ctx.cdp.run(truncationNoteProbe, root.root_id);
      check(!!big && big.rows > 0 && big.hasNote === true, `被截斷的 commit：詳情顯示「變更過多，只列出前面一部分」（實際 ${JSON.stringify(big)}）`);
      check(!!big && big.afterList, `提示位於檔案清單末端（在 .commit-detail-files 之後；實際 ${JSON.stringify(big)}）`);
    }
  );
}

// GIVEN commit X 與 Y 之間的變更檔案多到後端回報 truncated 為 true WHEN 以 X 為比較基準、點選 Y THEN
// 比較詳情的檔案清單末端顯示「變更過多，只列出前面一部分」；檔案數未被截斷的兩個 commit 比較時不顯示。
async function segCompareTruncationNote() {
  await withCockpit(
    'compare-truncated',
    {
      beforeLoad: (preview) => {
        preview.huge = addHugeChangeCommit(preview.reviewRepo, 'git-check-huge');
      },
    },
    async (ctx) => {
      const root = await rootInfo(ctx, PANE_REVIEW);
      const huge = ctx.preview.huge;
      const status = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/status`);
      need(status.status === 200 && status.body && status.body.branch, `前置：狀態端點回 200（實際 ${status.status}）`);
      const headOid = status.body.branch.oid;
      const refs = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/refs`);
      const fRef = refs.status === 200 && refs.body && Array.isArray(refs.body.refs) ? refs.body.refs.find((r) => r.kind === 'branch' && r.short === 'feature/logging') : null;
      need(!!fRef && /^[0-9a-f]{40}$/.test(fRef.oid), `前置：找到 feature/logging 分支與其 oid（實際 ${JSON.stringify(fRef)}）`);
      const apiBig = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/changes?from=${headOid}&to=${huge.oid}`);
      need(apiBig.status === 200 && apiBig.body && apiBig.body.truncated === true, `前置：changes 端點對 HEAD↔大量變更 commit 回 truncated=true（實際 ${apiBig.status} ${JSON.stringify(apiBig.body && apiBig.body.truncated)}）`);
      const apiSmall = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/changes?from=${headOid}&to=${fRef.oid}`);
      need(apiSmall.status === 200 && apiSmall.body && apiSmall.body.truncated === false, `前置：HEAD↔feature/logging 的 truncated 為 false（實際 ${JSON.stringify(apiSmall.body && apiSmall.body.truncated)}）`);

      await openChanges(ctx);
      await openGraphTab(ctx, root);
      const rowsLoaded = await ctx.cdp.poll((rid, a, b) => !!window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), a) && !!window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), b), [root.root_id, huge.oid, fRef.oid], UI_TIMEOUT_MS);
      need(!!rowsLoaded, '前置：大量變更 commit 與 feature/logging tip 列都已載入');

      need(await ctx.cdp.clickEl((rid, o) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), o), [root.root_id, headOid], 'HEAD 列'), '點 HEAD 列');
      const headReady = await ctx.cdp.poll(
        (rid) => {
          const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
          return !!t && !t.includes('正在讀取') && !!window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '選為比較基準');
        },
        [root.root_id],
        UI_TIMEOUT_MS
      );
      need(!!headReady, 'HEAD 的詳情載入完成');
      need(await ctx.cdp.clickEl((rid) => window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '選為比較基準'), [root.root_id], '「選為比較基準」按鈕'), '點「選為比較基準」按鈕');

      async function compareWith(oid, label) {
        need(await ctx.cdp.clickEl((rid, o) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), o), [root.root_id, oid], label), `點${label}`);
        const ready = await ctx.cdp.poll(
          (rid, o) => {
            const p = window.__gc.graphPanelOf(rid);
            const t = window.__gc.commitDetailText(p);
            return !!t && t.includes('比較') && t.includes(o.slice(0, 7)) && !!window.__gc.commitDetailButton(p, '直接比較') && !t.includes('正在讀取');
          },
          [root.root_id, oid],
          UI_TIMEOUT_MS
        );
        need(!!ready, `選取${label}後詳情為「比較」畫面且載入完成`);
      }

      await compareWith(fRef.oid, 'feature/logging tip 列');
      const normal = await ctx.cdp.run(truncationNoteProbe, root.root_id);
      check(!!normal && normal.rows > 0 && normal.hasNote === false, `未被截斷的比較（HEAD ↔ feature/logging）：不顯示「變更過多，只列出前面一部分」（實際 ${JSON.stringify(normal)}）`);

      await compareWith(huge.oid, '大量變更（被截斷）commit 列');
      const big = await ctx.cdp.run(truncationNoteProbe, root.root_id);
      check(!!big && big.rows > 0 && big.hasNote === true, `被截斷的比較（HEAD ↔ 大量變更 commit）：顯示「變更過多，只列出前面一部分」（實際 ${JSON.stringify(big)}）`);
      check(!!big && big.afterList, `提示位於比較詳情檔案清單末端（實際 ${JSON.stringify(big)}）`);
    }
  );
}

// GIVEN Git Graph 分頁為目前分頁 WHEN 在 repo 新增一個 commit（`git commit --allow-empty`，控制端
// 裁決的寫入方式）THEN 3 秒內出現「分支已變更」與「重新載入」按鈕，清單內容與捲動位置不變；按
// 「重新載入」後新 commit 出現在第一列。
async function segRefsChangedBanner() {
  await withCockpit('refs-changed', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await openChanges(ctx);
    await openGraphTab(ctx, root);
    const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
    need(!!loaded, '第一批 200 列載入完成');

    const before = await ctx.cdp.run((rid) => {
      const p = window.__gc.graphPanelOf(rid);
      const sc = window.__gc.graphScroller(p);
      sc.scrollTop = 40;
      return { firstOid: window.__gc.graphRows(p)[0].getAttribute('data-oid'), rows: window.__gc.graphRows(p).length, scrollTop: sc.scrollTop, bannerHidden: window.__gc.graphBanner(p).hidden };
    }, root.root_id);
    need(before.bannerHidden, `前置：一開始沒有「分支已變更」提示（實際 ${JSON.stringify(before)}）`);

    // git-review task 4.4 控制端裁決：在暫存副本 repo 新增 commit，先驗證路徑就是暫存副本本身
    // （gitTemp() 內建這個安全檢查）。
    verifyTempRepoToplevel(ctx.preview.reviewRepo);
    gitTemp(ctx.preview.reviewRepo, ['-c', 'user.name=git-check', '-c', 'user.email=git-check@invalid', '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'git-check: 分支變更提示測試']);
    const newOid = gitTemp(ctx.preview.reviewRepo, ['rev-parse', 'HEAD']);
    check(/^[0-9a-f]{40}$/.test(newOid), `新增 commit 成功、取得新 HEAD 的 40 碼 hash（${newOid}）`);

    const t0 = Date.now();
    const bannerShown = await ctx.cdp.poll((rid) => !window.__gc.graphBanner(window.__gc.graphPanelOf(rid)).hidden, [root.root_id], 3000);
    const elapsed = Date.now() - t0;
    check(!!bannerShown, `3 秒內出現「分支已變更」提示（${bannerShown ? `${elapsed} ms` : '逾時'}）`);
    const reloadBtnVisible = await ctx.cdp.run((rid) => {
      const btn = window.__gc.graphButtonByAction(window.__gc.graphPanelOf(rid), 'graph-reload');
      return !!btn && !btn.hidden;
    }, root.root_id);
    check(reloadBtnVisible, '「重新載入」按鈕出現');

    const stillSame = await ctx.cdp.run((rid) => {
      const p = window.__gc.graphPanelOf(rid);
      return { firstOid: window.__gc.graphRows(p)[0].getAttribute('data-oid'), rows: window.__gc.graphRows(p).length, scrollTop: window.__gc.graphScroller(p).scrollTop };
    }, root.root_id);
    check(stillSame.firstOid === before.firstOid && stillSame.rows === before.rows, `banner 出現期間清單內容不變（${JSON.stringify(before)} → ${JSON.stringify(stillSame)}）`);
    check(Math.abs(stillSame.scrollTop - before.scrollTop) <= 1, `banner 出現期間捲動位置不變（${before.scrollTop} → ${stillSame.scrollTop}）`);

    need(await ctx.cdp.clickEl((rid) => window.__gc.graphButtonByAction(window.__gc.graphPanelOf(rid), 'graph-reload'), [root.root_id], '「重新載入」按鈕'), '點「重新載入」按鈕');
    const reloaded = await ctx.cdp.poll(
      (rid, oid) => {
        const p = window.__gc.graphPanelOf(rid);
        const rows = window.__gc.graphRows(p);
        return rows.length > 0 && rows[0].getAttribute('data-oid') === oid;
      },
      [root.root_id, newOid],
      UI_TIMEOUT_MS
    );
    check(!!reloaded, `按「重新載入」後新 commit 出現在第一列（預期 ${newOid}）`);
    const bannerClearedAfter = await ctx.cdp.run((rid) => window.__gc.graphBanner(window.__gc.graphPanelOf(rid)).hidden, root.root_id);
    check(bannerClearedAfter, '重新載入後「分支已變更」提示清除');
  });
}

// GIVEN Git Graph 分頁為左欄「變更」分頁點出的目前分頁、清單已捲動到非 0 位置、已選取並聚焦某一
// 列 WHEN ui_preview 觸發至少 3 次整頁重畫（投影推送）THEN 捲動位置、選取與焦點都不變、清單 DOM
// 節點沒被換掉（同 git-review task 4.2 fix round 2「重畫不影響變更分頁」的驗法：`#review` 底下的
// 節點在 `#app` 之外，整頁重畫的 `replaceChildren()` 本來就碰不到，這裡用同一套量測工具機械驗證
// 這個結論在 Git Graph 分頁一樣成立）。
async function segRepaintKeepsGraph() {
  await withCockpit('graph-repaint', { env: { COCKPIT_PREVIEW_PUSH_MS: '100' } }, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await openChanges(ctx);
    await openGraphTab(ctx, root);
    const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
    need(!!loaded, '第一批 200 列載入完成');

    const setup = await ctx.cdp.run((rid) => {
      const p = window.__gc.graphPanelOf(rid);
      const sc = window.__gc.graphScroller(p);
      sc.scrollTop = Math.min(60, sc.scrollHeight - sc.clientHeight);
      const rows = window.__gc.graphRows(p);
      const target = rows[3];
      target.focus({ preventScroll: true });
      target.click(); // 選取（見 git.js selectGraphRow()）
      window.__gcFocusTarget = target;
      return { scrollTop: sc.scrollTop, max: sc.scrollHeight - sc.clientHeight, focused: document.activeElement === target, selected: target.getAttribute('aria-selected') === 'true', oid: target.getAttribute('data-oid') };
    }, root.root_id);
    need(!!setup && setup.scrollTop > 0 && setup.focused && setup.selected, `前置：Git Graph 清單捲到非 0 位置、第 4 列被選取並聚焦（${JSON.stringify(setup)}）`);

    // git-review task 4.5：點選現在會在該列下方非同步展開 commit 詳情（commit 端點的讀取），必須等它
    // 讀完（詳情文字不再是「正在讀取…」）才能拍快照——否則詳情讀取完成時機落在快照之後、重畫等待
    // 之前，會被 graphCompare() 的「子孫節點是否相同」誤判成「清單被改動」（其實只是詳情內容從
    // 「正在讀取」換成完整內容，跟本段要驗的整頁重畫無關）。
    const detailReady = await ctx.cdp.poll(
      (rid) => {
        const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
        return !!t && !t.includes('正在讀取');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!detailReady, '前置：點選觸發的 commit 詳情已載入完成（快照要在它穩定之後才拍）');

    const snap = await ctx.cdp.run((rid) => window.__gc.graphSnapshot(rid), root.root_id);
    need(!snap.error, `前置：記下 Git Graph 清單、每一列與捲動容器（${JSON.stringify(snap)}）`);

    // 同 segRepaintKeepsChanges()：不先驗「Project 是否隱藏」當前置條件，直接進「至少 3 次重畫」
    // 迴圈，最後用 check() 驗最終狀態（這個 bug 的表現正是「重畫後才冒出來」，見 fix round 2）。
    const v0 = await ctx.cdp.run(() => document.getElementById('version').getAttribute('data-state-version'));
    let repaints = 0;
    let lastV = v0;
    const start = Date.now();
    while (Date.now() - start < 5000 && repaints < 3) {
      await sleep(100);
      const v = await ctx.cdp.run(() => document.getElementById('version').getAttribute('data-state-version'));
      if (v !== lastV) {
        repaints += 1;
        lastV = v;
      }
    }
    need(repaints >= 3, `前置：期間至少發生 3 次整頁重畫（#version 的 data-state-version 變化 ${repaints} 次）`);

    const projectsHiddenAfter = await ctx.cdp.run(() => window.__gc.projectsHidden());
    check(projectsHiddenAfter === true, `重畫 ${repaints} 次後 Project 清單仍隱藏（實際 hidden=${projectsHiddenAfter}）`);

    const after = await ctx.cdp.run((rid) => window.__gc.graphCompare(rid), root.root_id);
    check(after.sameList && after.sameRows && after.sameDescendants, `Git Graph 清單的 DOM 節點沒有被換掉（list ${after.sameList}、列 ${after.sameRows}、列內子孫 ${after.sameDescendants}；${after.descendants} 個節點）`);
    check(after.scSame, `捲動容器仍是原本那一個且仍在頁面上（${JSON.stringify({ scSame: after.scSame })}）`);
    check(after.scrollTop !== null && Math.abs(after.scrollTop - after.expectScrollTop) <= 1, `捲動位置不變（${after.expectScrollTop} → ${after.scrollTop}）`);

    const focusAndSelection = await ctx.cdp.run((rid, oid) => {
      const p = window.__gc.graphPanelOf(rid);
      const row = window.__gc.graphRowByOid(p, oid);
      return {
        focusSame: document.activeElement === window.__gcFocusTarget,
        stillSelected: !!row && row.getAttribute('aria-selected') === 'true',
        rowIsSameNode: row === window.__gcFocusTarget,
      };
    }, root.root_id, setup.oid);
    check(focusAndSelection.focusSame, '焦點仍在原本的列上');
    check(focusAndSelection.stillSelected && focusAndSelection.rowIsSameNode, `選取狀態不變、列的 DOM 節點也沒被換掉（實際 ${JSON.stringify(focusAndSelection)}）`);
  });
}

// ---------------------------------------------------------------------------
// git-review：commit 詳情、比較與某版本檔案分頁（git-review task 4.5；spec「commit 詳情與比較」
// 「某版本檔案分頁」；design D9）
// ---------------------------------------------------------------------------
//
// 五段都用同一份 review-repo fixture 的既定歷史（design D10；task 3.1 fixture；見 git-check.md）：
//   - main 的 HEAD 是合併 commit E（「合併 feature/formatting 回 main」），第一個 parent 是 main 上
//     緊接分支點之後的 D（「分支點之後 main 再前進一個 commit」），第二個 parent 是 feature/formatting
//     的 tip C；E、D、C 依日期排序落在 Git Graph 第一批（<200）的最前幾列，不需要載入更多。
//   - feature/logging 的 tip F（「feature/logging：第二個 commit」）與 feature/formatting 的 tip C
//     從同一個「分支點」B（「新增工作區情境要用到的追蹤檔案」commit）分岔；B 是 E 與 F 的共同祖先，
//     且 B ≠ E、B ≠ F（真正測到「自分岔點起」與「直接比較」的差異，不是巧合地相同）。
//   - `history/counter.txt` 在 main 的歷史上被逐一填充 commit（`#1`…`#250`）與最後一次
//     「post-branch」改寫，內容隨版本改變——用它驗「看舊版規格」讀到的是「當時」的內容，不是
//     工作區現在的內容。填充 commit #210（訊息「填充 commit #210（git-review task 3.1 fixture）」）
//     落在 Git Graph 第一批之內（第一批 200 列由 E 往回數，第 210 個 filler 落在第 53 列左右），
//     不需要先載入更多；搜尋字串固定用「commit #210」（含空白與 #，不會誤中 #21、#2100 這類子字串——
//     全部 filler 最多到 #250，三位數不會出現「210」以外還含「commit #210」子字串的訊息）。

// 開啟「填充 commit #210」的 history/counter.txt 某版本分頁（segOldVersion／segCommitVersionNoPoll
// 共用的前置流程）：開 Git Graph → 找到該列並點開詳情 → 點 history/counter.txt 的「看此版本」。
// 回傳該 commit 的 40 碼 hash。
async function openOldCounterRevTab(ctx, root) {
  await openGraphTab(ctx, root);
  const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
  need(!!loaded, '前置：第一批 200 列載入完成');

  const rowOid = await ctx.cdp.run((rid) => {
    const p = window.__gc.graphPanelOf(rid);
    const r = window.__gc.graphRowBySubject(p, 'commit #210');
    return r ? r.getAttribute('data-oid') : null;
  }, root.root_id);
  need(!!rowOid && /^[0-9a-f]{40}$/.test(rowOid), `前置：找到「填充 commit #210」列並取得其 40 碼 hash（實際 ${rowOid}）`);

  need(
    await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, rowOid], '填充 commit #210 列'),
    '點填充 commit #210 列'
  );
  const detailReady = await ctx.cdp.poll(
    (rid) => {
      const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
      return !!t && !t.includes('正在讀取');
    },
    [root.root_id],
    UI_TIMEOUT_MS
  );
  need(!!detailReady, '前置：填充 commit #210 的詳情載入完成');

  need(
    await ctx.cdp.clickEl(
      (rid) => {
        const row = window.__gc.commitDetailFileRow(window.__gc.graphPanelOf(rid), 'history/counter.txt');
        return row ? Array.from(row.querySelectorAll('button')).find((b) => window.__gc.txt(b) === '看此版本') : null;
      },
      [root.root_id],
      'history/counter.txt 的「看此版本」按鈕'
    ),
    '點 history/counter.txt 的「看此版本」按鈕'
  );
  const opened = await ctx.cdp.poll(
    (p, rev) => {
      const t = window.__gc.revTab(p, rev);
      return !!t && t.getAttribute('aria-selected') === 'true';
    },
    ['history/counter.txt', rowOid],
    UI_TIMEOUT_MS
  );
  need(!!opened, `前置：history/counter.txt 的某版本分頁（@ ${rowOid.slice(0, 7)}）出現並成為目前分頁`);
  return rowOid;
}

// GIVEN Git Graph 已載入 WHEN 點修改了 history/feature-formatting.txt 的 merge commit（HEAD），再點
// 詳情中的 history/feature-formatting.txt THEN 詳情顯示完整 hash 與訊息並列出該檔案（含「與第一個父
// commit 比較」註記，因為是 merge commit）；分頁區出現該檔案的 diff 分頁，左側為該 commit 的第一個
// parent、右側為該 commit。
async function segCommitDetailOpenDiff() {
  await withCockpit('commit-detail-diff', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    const status = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/status`);
    need(status.status === 200 && status.body && status.body.branch && typeof status.body.branch.oid === 'string', `前置：狀態端點回 200 並帶 branch.oid（實際 ${status.status} ${JSON.stringify(status.body)}）`);
    const headOid = status.body.branch.oid;
    const commitInfo = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/commit/${headOid}`);
    need(
      commitInfo.status === 200 && commitInfo.body && Array.isArray(commitInfo.body.parents) && commitInfo.body.parents.length === 2 && typeof commitInfo.body.compared_to === 'string',
      `前置：HEAD 是 merge commit（2 個 parents）且 compared_to 是第一個 parent 的 hash（實際 ${commitInfo.status} ${JSON.stringify(commitInfo.body)}）`
    );
    const comparedTo = commitInfo.body.compared_to;
    need(
      commitInfo.body.files.some((f) => f.path === 'history/feature-formatting.txt'),
      `前置：HEAD 與第一個 parent 之間 history/feature-formatting.txt 有差異（fixture 既定歷史；實際 files=${JSON.stringify(commitInfo.body.files.map((f) => f.path))}）`
    );

    await openChanges(ctx);
    await openGraphTab(ctx, root);
    const rowLoaded = await ctx.cdp.poll((rid, oid) => !!window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, headOid], UI_TIMEOUT_MS);
    need(!!rowLoaded, `前置：HEAD 列（${headOid.slice(0, 7)}）已載入`);
    need(
      await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, headOid], 'HEAD（merge commit）列'),
      '點 HEAD（merge commit）列'
    );
    const detailReady = await ctx.cdp.poll(
      (rid) => {
        const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
        return !!t && !t.includes('正在讀取');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!detailReady, 'HEAD 的詳情載入完成');

    const detailText = await ctx.cdp.run((rid) => window.__gc.commitDetailText(window.__gc.graphPanelOf(rid)), root.root_id);
    check(detailText.includes(headOid), `詳情顯示完整 hash（實際含 ${detailText.includes(headOid)}）`);
    check(detailText.includes('合併 feature/formatting 回 main'), `詳情顯示完整訊息（實際「${detailText.slice(0, 200)}」）`);
    check(detailText.includes('與第一個父 commit 比較'), 'merge commit 的詳情註明「與第一個父 commit 比較」');

    const fileRowExists = await ctx.cdp.run((rid) => !!window.__gc.commitDetailFileRow(window.__gc.graphPanelOf(rid), 'history/feature-formatting.txt'), root.root_id);
    check(fileRowExists, '詳情的變更檔案清單列出 history/feature-formatting.txt');

    need(
      await ctx.cdp.clickEl((rid) => window.__gc.commitDetailFileRow(window.__gc.graphPanelOf(rid), 'history/feature-formatting.txt'), [root.root_id], '詳情中的 history/feature-formatting.txt 列'),
      '點詳情中的 history/feature-formatting.txt 列'
    );
    const diffOpened = await ctx.cdp.poll(
      (p, from, to) => {
        const t = window.__gc.diffTab(p, from, to);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/feature-formatting.txt', comparedTo, headOid],
      UI_TIMEOUT_MS
    );
    check(!!diffOpened, `分頁區出現並選定 history/feature-formatting.txt 的 diff 分頁（左側 ${comparedTo.slice(0, 7)} → 右側 ${headOid.slice(0, 7)}）`);
  });
}

// GIVEN Git Graph 已載入 WHEN 點 commit E（HEAD）、按「選為比較基準」，再點 commit F（feature/logging
// 的 tip）THEN 詳情顯示「比較 E ↔ F」與兩者之間的變更檔案；切到「自分岔點起」後，檔案清單改為兩者
// 共同祖先 B 與 F 之間的變更（只有 history/feature-logging.txt——B 到 F 之間唯一有差異的檔案，跟
// 「直接比較」的檔案清單明顯不同，證明兩種模式真的各自查詢，不是巧合地一樣）。
async function segCompareCommits() {
  await withCockpit('compare-commits', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    const status = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/status`);
    need(status.status === 200 && status.body && status.body.branch, `前置：狀態端點回 200（實際 ${status.status}）`);
    const eOid = status.body.branch.oid;
    const refs = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/refs`);
    need(refs.status === 200 && refs.body && Array.isArray(refs.body.refs), `前置：refs 端點回 200（實際 ${refs.status}）`);
    const fRef = refs.body.refs.find((r) => r.kind === 'branch' && r.short === 'feature/logging');
    need(!!fRef && /^[0-9a-f]{40}$/.test(fRef.oid), `前置：找到 feature/logging 分支與其 oid（實際 ${JSON.stringify(fRef)}）`);
    const fOid = fRef.oid;
    const mergeBase = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/merge-base?a=${eOid}&b=${fOid}`);
    need(mergeBase.status === 200 && mergeBase.body && typeof mergeBase.body.oid === 'string', `前置：E 與 F 有共同祖先（實際 ${mergeBase.status} ${JSON.stringify(mergeBase.body)}）`);
    const bOid = mergeBase.body.oid;
    need(bOid !== eOid && bOid !== fOid, `前置：共同祖先 B 跟 E、F 都不同（實際 B=${bOid.slice(0, 7)} E=${eOid.slice(0, 7)} F=${fOid.slice(0, 7)}）`);
    const forkChanges = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/changes?from=${bOid}&to=${fOid}`);
    need(
      forkChanges.status === 200 && Array.isArray(forkChanges.body.files) && forkChanges.body.files.length === 1 && forkChanges.body.files[0].path === 'history/feature-logging.txt',
      `前置：B 到 F 之間只有 history/feature-logging.txt 一個檔案（fixture 既定歷史；實際 ${JSON.stringify(forkChanges.body)}）`
    );

    await openChanges(ctx);
    await openGraphTab(ctx, root);
    const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
    need(!!loaded, '前置：第一批 200 列載入完成（E、F 都在其中）');

    need(await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, eOid], 'commit E（HEAD）列'), '點 commit E（HEAD）列');
    const eDetailReady = await ctx.cdp.poll((rid) => {
      const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
      return !!t && !t.includes('正在讀取');
    }, [root.root_id], UI_TIMEOUT_MS);
    need(!!eDetailReady, 'commit E 的詳情載入完成');

    need(await ctx.cdp.clickEl((rid) => window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '選為比較基準'), [root.root_id], '「選為比較基準」按鈕'), '點「選為比較基準」按鈕');

    need(await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, fOid], 'commit F（feature/logging tip）列'), '點 commit F（feature/logging tip）列');
    const compareReady = await ctx.cdp.poll(
      (rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const t = window.__gc.commitDetailText(p);
        return !!t && t.includes('比較') && !!window.__gc.commitDetailButton(p, '直接比較') && !t.includes('正在讀取');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!compareReady, '選取 F 後詳情改為「比較」畫面（已有比較基準時選取另一個 commit）');

    const headerText = await ctx.cdp.run((rid) => window.__gc.commitDetailText(window.__gc.graphPanelOf(rid)), root.root_id);
    check(headerText.includes(eOid.slice(0, 7)) && headerText.includes(fOid.slice(0, 7)), `詳情顯示「比較 <E 短 hash> ↔ <F 短 hash>」（實際「${headerText.slice(0, 120)}」）`);

    const directToggle = await ctx.cdp.run((rid) => window.__gc.commitDetailToggle(window.__gc.graphPanelOf(rid), '直接比較'), root.root_id);
    check(!!directToggle && directToggle.pressed === 'true', `「直接比較」預設生效中（實際 ${JSON.stringify(directToggle)}）`);
    const directPaths = await ctx.cdp.run((rid) => window.__gc.commitDetailFilePaths(window.__gc.graphPanelOf(rid)), root.root_id);
    check(directPaths.length > 1, `「直接比較」（E ↔ F）列出的變更檔案不只一個（實際 ${directPaths.length} 個：${JSON.stringify(directPaths)}）`);
    check(directPaths.includes('history/feature-logging.txt'), `「直接比較」的檔案清單含 history/feature-logging.txt（實際 ${JSON.stringify(directPaths)}）`);

    need(await ctx.cdp.clickEl((rid) => window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '自分岔點起'), [root.root_id], '「自分岔點起」按鈕'), '點「自分岔點起」按鈕');
    const forkReady = await ctx.cdp.poll(
      (rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const toggle = window.__gc.commitDetailToggle(p, '自分岔點起');
        return !!toggle && toggle.pressed === 'true' && !window.__gc.commitDetailText(p).includes('正在讀取');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!forkReady, '切到「自分岔點起」後重新查詢完成');
    const forkPaths = await ctx.cdp.run((rid) => window.__gc.commitDetailFilePaths(window.__gc.graphPanelOf(rid)), root.root_id);
    check(forkPaths.length === 1 && forkPaths[0] === 'history/feature-logging.txt', `切到「自分岔點起」後，檔案清單改為共同祖先 B 與 F 之間的變更（只有 history/feature-logging.txt；實際 ${JSON.stringify(forkPaths)}）`);
    check(JSON.stringify(forkPaths) !== JSON.stringify(directPaths), `「自分岔點起」與「直接比較」的檔案清單確實不同（不是巧合地相同）`);

    need(
      await ctx.cdp.clickEl((rid) => window.__gc.commitDetailFileRow(window.__gc.graphPanelOf(rid), 'history/feature-logging.txt'), [root.root_id], '「自分岔點起」清單中的 history/feature-logging.txt 列'),
      '點「自分岔點起」清單中的 history/feature-logging.txt 列'
    );
    const diffOpened = await ctx.cdp.poll(
      (p, from, to) => {
        const t = window.__gc.diffTab(p, from, to);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/feature-logging.txt', bOid, fOid],
      UI_TIMEOUT_MS
    );
    check(!!diffOpened, `點選開啟對應的 diff 分頁（左側 B=${bOid.slice(0, 7)} → 右側 F=${fOid.slice(0, 7)}）`);
  });
}

// WHEN 在詳情按完整 hash 旁的「複製」THEN 剪貼簿內容為該 commit 的 40 字元 hash，畫面顯示「已複製」；
// 2 秒後提示消失。CDP 以 Browser.grantPermissions 授予剪貼簿權限（headless Chrome 預設會擋
// navigator.clipboard 的存取，沒有這個授權，寫入與讀回都會失敗，測不出真正的複製行為）。
async function segCopyHash() {
  await withCockpit('copy-hash', {}, async (ctx) => {
    const grant = await ctx.cdp.send('Browser.grantPermissions', { origin: ctx.origin, permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'] });
    need(!grant || !grant.error, `前置：CDP 授予剪貼簿權限成功（實際 ${JSON.stringify(grant)}）`);

    const root = await rootInfo(ctx, PANE_REVIEW);
    const status = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/status`);
    need(status.status === 200 && status.body && status.body.branch, `前置：狀態端點回 200（實際 ${status.status}）`);
    const headOid = status.body.branch.oid;

    await openChanges(ctx);
    await openGraphTab(ctx, root);
    const rowLoaded = await ctx.cdp.poll((rid, oid) => !!window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, headOid], UI_TIMEOUT_MS);
    need(!!rowLoaded, `前置：HEAD 列（${headOid.slice(0, 7)}）已載入`);
    need(await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, headOid], 'HEAD 列'), '點 HEAD 列');
    const detailReady = await ctx.cdp.poll(
      (rid) => {
        const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
        return !!t && !t.includes('正在讀取');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!detailReady, 'HEAD 的詳情載入完成');

    need(
      await ctx.cdp.clickEl(
        (rid) => {
          const w = window.__gc.commitDetailWrap(window.__gc.graphPanelOf(rid));
          return w ? w.querySelector('[aria-label="複製完整 hash"]') : null;
        },
        [root.root_id],
        '完整 hash 旁的「複製」按鈕'
      ),
      '點完整 hash 旁的「複製」按鈕'
    );
    const t0 = Date.now();
    const shown = await ctx.cdp.poll((rid) => window.__gc.copyFeedbackText(window.__gc.graphPanelOf(rid)) === '已複製', [root.root_id], UI_TIMEOUT_MS);
    check(!!shown, `畫面顯示「已複製」（${shown ? `${Date.now() - t0} ms` : '逾時未出現'}）`);

    const clipboardText = await ctx.cdp.eval('navigator.clipboard.readText()');
    check(clipboardText === headOid, `剪貼簿內容為該 commit 的 40 字元 hash（預期 ${headOid}，實際 ${clipboardText}）`);

    await sleep(2300);
    const goneAfter = await ctx.cdp.run((rid) => window.__gc.copyFeedbackText(window.__gc.graphPanelOf(rid)), root.root_id);
    check(goneAfter === null, `提示在 2 秒後消失（實際 ${JSON.stringify(goneAfter)}）`);
  });
}

// GIVEN history/counter.txt 在填充 commit #210 時的內容為「210」，工作區為「post-branch」（main 上
// 最後一次改寫）WHEN 在該 commit 的詳情中按 history/counter.txt 的「看此版本」THEN 出現
// 「counter.txt @ <該 commit 短 hash>」分頁，以文字檢視器顯示該版本當時的內容（含「210」、不含
// 「post-branch」——證明讀到的是歷史版本、不是工作區現在的內容）。spec 原文的例子是 Markdown
// （plan.md），這裡改用 history/counter.txt（.txt，文字檢視器）：Markdown 相對連結／圖片改寫的行為
// 完全交給 viewers.js（本 task 沒有改動 viewers.js 一行，見報告），四種檢視器共用同一套 ctx 契約，
// 用文字檢視器驗證「某版本檔案分頁」本身的機制（正確的版本、正確的內容、正確的標籤）已經足夠；
// Markdown 檢視器的畫面另外在截圖裡示範（見報告「截圖」）。
async function segOldVersion() {
  await withCockpit('old-version', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await openChanges(ctx);
    const rowOid = await openOldCounterRevTab(ctx, root);

    const ready = await ctx.cdp.poll(
      (p, rev) => {
        const host = window.__gc.revHost(window.__gc.revPanel(p, rev));
        return !!host && host.textContent.includes('210');
      },
      ['history/counter.txt', rowOid],
      UI_TIMEOUT_MS
    );
    need(!!ready, '某版本分頁內容顯示該版本當時的內容（含「210」）');

    const info = await ctx.cdp.run(
      (p, rev) => {
        const panel = window.__gc.revPanel(p, rev);
        const host = window.__gc.revHost(panel);
        const viewerEl = host ? host.querySelector('[data-viewer]') : null;
        const t = window.__gc.revTab(p, rev);
        return { viewer: viewerEl ? viewerEl.getAttribute('data-viewer') : null, text: host ? host.textContent : '', tabLabel: t ? window.__gc.txt(t) : null };
      },
      'history/counter.txt',
      rowOid
    );
    check(info.viewer === 'text', `以文字檢視器顯示（實際 ${info.viewer}）`);
    check(info.text.includes('210') && !info.text.includes('post-branch'), `內容為該版本當時的內容（含 210、不含之後才改寫的 post-branch；實際片段「${info.text.slice(0, 60)}」）`);
    check(info.tabLabel.includes('counter.txt') && info.tabLabel.includes(rowOid.slice(0, 7)), `分頁標籤含檔名與短 hash（實際「${info.tabLabel}」）`);

    need(
      await ctx.cdp.clickEl((p, rev) => window.__gc.revButton(window.__gc.revPanel(p, rev), '開啟目前版本'), ['history/counter.txt', rowOid], '「開啟目前版本」按鈕'),
      '點「開啟目前版本」按鈕'
    );
    const currentOpened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.fileTab(p);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/counter.txt'],
      UI_TIMEOUT_MS
    );
    check(!!currentOpened, '「開啟目前版本」開啟並選定工作區版本的檔案分頁');
  });
}

// GIVEN 某版本檔案分頁（commit 版本）為目前分頁 WHEN 觀察 10 秒 THEN 服務在初次載入之後沒有再收到
// 該分頁的任何請求。
async function segCommitVersionNoPoll() {
  await withCockpit('commit-no-poll', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await openChanges(ctx);
    const rowOid = await openOldCounterRevTab(ctx, root);
    const ready = await ctx.cdp.poll(
      (p, rev) => {
        const host = window.__gc.revHost(window.__gc.revPanel(p, rev));
        return !!host && host.textContent.includes('210');
      },
      ['history/counter.txt', rowOid],
      UI_TIMEOUT_MS
    );
    need(!!ready, '前置：內容載入完成');

    const t0 = Date.now();
    await sleep(10000);
    const relevant = ctx.net.since(t0).filter((r) => r.url.includes(rowOid) && r.url.includes('counter.txt'));
    check(relevant.length === 0, `觀察 10 秒，服務沒有再收到該分頁（${rowOid.slice(0, 7)}／history/counter.txt）的任何請求（實際 ${relevant.length} 次：${JSON.stringify(relevant.map((r) => r.url))}）`);
  });
}

// ---------------------------------------------------------------------------
// file-review：分頁還原（git-review task 4.5；spec file-review「分頁還原」新情境「還原 git 分頁與
// 變更分頁」；design D9 v:2 格式）——放在本檔而不是 files-check.js，因為需要 diff／Git Graph 分頁與
// review-repo 的 git fixture，這裡已經有現成的機器（見 git-check.md／brief 控制端裁決）。
// ---------------------------------------------------------------------------

// 分頁列目前每一個分頁的簡短識別字串（LIVE／檔名／`diff:<path>`／graph），依分頁順序；用來比對
// 「還原後順序不變」，不需要 files-check.js 的 `tabsInfo()`（那支只認得 file kind）。
function tabKindList() {
  return window.__gc.reviewTabs().map((t) => {
    if (window.__gc.isLive(t)) return 'LIVE';
    if (t.getAttribute('data-path')) return t.getAttribute('data-path');
    if (t.getAttribute('data-diff-path')) return `diff:${t.getAttribute('data-diff-path')}`;
    if (t.getAttribute('data-graph-root')) return 'graph';
    if (t.getAttribute('data-rev-path')) return `rev:${t.getAttribute('data-rev-path')}`;
    return '?';
  });
}

// GIVEN 已打開 README.md、history/unstaged-change.txt 的 diff 分頁（變更）與 Git Graph 分頁，目前
// 分頁為 Git Graph，左欄目前為「變更」WHEN 重新整理頁面 THEN 三個分頁依原順序還原，目前分頁為
// Git Graph 且顯示其內容，左欄為「變更」分頁。
async function segRestoreGitAndChangesTabs() {
  await withCockpit('restore-git-tabs', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);

    await selectPane(ctx, PANE_REVIEW);
    await switchLeftTab(ctx, '檔案');
    const readmeVisible = await ctx.cdp.poll(() => !!document.querySelector('#files [role="treeitem"][title="README.md"]'), [], UI_TIMEOUT_MS);
    need(!!readmeVisible, '前置：檔案樹列出 README.md');
    need(await ctx.cdp.clickEl(() => document.querySelector('#files [role="treeitem"][title="README.md"]'), [], 'README.md 檔案樹列'), '點 README.md 檔案樹列');
    const readmeOpened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.fileTab(p);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['README.md'],
      UI_TIMEOUT_MS
    );
    need(!!readmeOpened, 'README.md 檔案分頁出現並成為目前分頁');

    await openChanges(ctx);
    await waitChangesRow(ctx, 'history/unstaged-change.txt');
    need(
      await ctx.cdp.clickEl((p) => window.__gc.changesRow(p), ['history/unstaged-change.txt'], '「變更」清單的 history/unstaged-change.txt 列'),
      '點「變更」清單的 history/unstaged-change.txt 列'
    );
    const diffOpened = await ctx.cdp.poll(
      (p, from, to) => {
        const t = window.__gc.diffTab(p, from, to);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/unstaged-change.txt', 'INDEX', 'WORKTREE'],
      UI_TIMEOUT_MS
    );
    need(!!diffOpened, 'history/unstaged-change.txt 的 diff 分頁出現並成為目前分頁');

    await openGraphTab(ctx, root);
    need((await ctx.cdp.run(() => window.__gc.leftSelected())) === '變更', '前置：開完三個分頁後左欄仍為「變更」');

    const order = await ctx.cdp.run(tabKindList);
    need(
      JSON.stringify(order) === JSON.stringify(['LIVE', 'README.md', 'diff:history/unstaged-change.txt', 'graph']),
      `前置：分頁依開啟順序排列（實際 ${JSON.stringify(order)}）`
    );

    await sleep(500); // 讓 persistTabs() 的寫入穩定（同 files-check.js segRestoreAfterReload() 的既有做法）
    await ctx.cdp.send('Page.reload', { ignoreCache: false });
    await waitForFirstProjection(ctx.cdp, ctx.preview.port, '重新整理後首份投影畫完');

    const restored = await ctx.cdp.poll(() => window.__gc.reviewTabs().length >= 4, [], UI_TIMEOUT_MS);
    if (!restored) {
      need(false, `重新整理後應該還原三個分頁；目前 DOM：${await contractDump(ctx)}`);
    }
    const orderAfter = await ctx.cdp.run(tabKindList);
    check(JSON.stringify(orderAfter) === JSON.stringify(order), `三個分頁依原順序還原（實際 ${JSON.stringify(orderAfter)}）`);

    const currentInfo = await ctx.cdp.run((rid) => {
      const t = window.__gc.selectedReviewTab();
      return t ? { isGraph: t.getAttribute('data-graph-root') === rid } : null;
    }, root.root_id);
    check(!!currentInfo && currentInfo.isGraph, `目前分頁為 Git Graph（實際 ${JSON.stringify(currentInfo)}）`);

    const graphContentReady = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length > 0, [root.root_id], UI_TIMEOUT_MS);
    check(!!graphContentReady, 'Git Graph 分頁顯示其內容（commit 清單重新載入）');

    check((await ctx.cdp.run(() => window.__gc.leftSelected())) === '變更', '左欄為「變更」分頁');

    // 額外驗證（超出 spec 逐字的三分頁 scenario；brief 控制端裁決：確認 diff、graph、rev 與左欄
    // changes 全部能還原）：從 diff 分頁的「看左側版本」再開一個 rev 分頁，重新整理一次，確認四個
    // 分頁（含 rev）都還原。
    need(
      await ctx.cdp.clickEl((p, from, to) => window.__gc.diffTab(p, from, to), ['history/unstaged-change.txt', 'INDEX', 'WORKTREE'], '切回 history/unstaged-change.txt 的 diff 分頁'),
      '切回 history/unstaged-change.txt 的 diff 分頁'
    );
    need(
      await ctx.cdp.clickEl((p, from, to) => window.__gc.diffToolbarButton(window.__gc.diffPanel(p, from, to), '看左側版本'), ['history/unstaged-change.txt', 'INDEX', 'WORKTREE'], '「看左側版本」按鈕'),
      '點「看左側版本」按鈕（開 rev 分頁）'
    );
    const revOpened = await ctx.cdp.poll(
      (p, rev) => {
        const t = window.__gc.revTab(p, rev);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/unstaged-change.txt', 'INDEX'],
      UI_TIMEOUT_MS
    );
    need(!!revOpened, 'rev 分頁（INDEX／history/unstaged-change.txt）出現並成為目前分頁');

    const orderWithRev = await ctx.cdp.run(tabKindList);
    await sleep(500);
    await ctx.cdp.send('Page.reload', { ignoreCache: false });
    await waitForFirstProjection(ctx.cdp, ctx.preview.port, '第二次重新整理後首份投影畫完（含 rev 分頁）');
    const restoredWithRev = await ctx.cdp.poll((n) => window.__gc.reviewTabs().length >= n, [orderWithRev.length], UI_TIMEOUT_MS);
    if (!restoredWithRev) {
      need(false, `重新整理後應該還原含 rev 分頁在內的全部分頁；目前 DOM：${await contractDump(ctx)}`);
    }
    const orderWithRevAfter = await ctx.cdp.run(tabKindList);
    check(JSON.stringify(orderWithRevAfter) === JSON.stringify(orderWithRev), `含 rev 分頁在內的分頁依原順序還原（實際 ${JSON.stringify(orderWithRevAfter)}）`);
    const revCurrentInfo = await ctx.cdp.run((p, rev) => {
      const t = window.__gc.revTab(p, rev);
      const cur = window.__gc.selectedReviewTab();
      return { isRev: !!t, isCurrent: !!t && t === cur };
    }, 'history/unstaged-change.txt', 'INDEX');
    check(revCurrentInfo.isRev && revCurrentInfo.isCurrent, `rev 分頁還原且仍為目前分頁（實際 ${JSON.stringify(revCurrentInfo)}）`);
  });
}

// ---------------------------------------------------------------------------
// code review 缺陷回歸（M1／M2／M3；控制端已讀程式碼確認）
// ---------------------------------------------------------------------------

// M1：Git Graph 分頁存在但不是目前分頁（例如重整頁面後還原、目前分頁是別的）時，scroller 的
// scrollHeight／clientHeight／scrollTop 都是 0，舊的自動載入判斷「0-0-0 <= 門檻」成立，會背景一路載到
// 上限。GIVEN 還原後 graph 分頁非目前分頁 WHEN 等 3 秒 THEN 只有第一批一次 /log 請求；切回並捲到底之後仍能載完。
async function segHiddenGraphNoAutoLoad() {
  await withCockpit('hidden-graph-no-autoload', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await selectPane(ctx, PANE_REVIEW);
    await switchLeftTab(ctx, '檔案');
    const readmeVisible = await ctx.cdp.poll(() => !!document.querySelector('#files [role="treeitem"][title="README.md"]'), [], UI_TIMEOUT_MS);
    need(!!readmeVisible, '前置：檔案樹列出 README.md');
    need(await ctx.cdp.clickEl(() => document.querySelector('#files [role="treeitem"][title="README.md"]'), [], 'README.md 檔案樹列'), '點 README.md 檔案樹列');
    const readmeOpened = await ctx.cdp.poll(
      (p) => {
        const t = window.__gc.fileTab(p);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['README.md'],
      UI_TIMEOUT_MS
    );
    need(!!readmeOpened, 'README.md 檔案分頁成為目前分頁');
    await openChanges(ctx);
    await openGraphTab(ctx, root);
    need(await ctx.cdp.clickEl(() => window.__gc.fileTab('README.md'), [], 'README.md 分頁'), '切回 README.md 分頁（讓 Git Graph 分頁變成非目前分頁）');
    await sleep(500); // 讓 persistTabs() 寫入穩定
    const t0 = Date.now();
    await ctx.cdp.send('Page.reload', { ignoreCache: false });
    await waitForFirstProjection(ctx.cdp, ctx.preview.port, '重新整理後首份投影畫完');
    const restored = await ctx.cdp.poll((rid) => !!window.__gc.graphTab(rid), [root.root_id], UI_TIMEOUT_MS);
    need(!!restored, '重新整理後還原 Git Graph 分頁');
    const cur = await ctx.cdp.run(() => {
      const t = window.__gc.selectedReviewTab();
      return t ? t.getAttribute('data-path') : null;
    });
    need(cur === 'README.md', `前置：還原後目前分頁是 README.md、Git Graph 分頁不是目前分頁（實際 ${JSON.stringify(cur)}）`);
    const firstBatch = await ctx.cdp.poll(
      (rid) => {
        const p = window.__gc.graphPanelOf(rid);
        return !!p && window.__gc.graphRows(p).length >= 200;
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!firstBatch, '隱藏的 Git Graph 分頁仍載入第一批（200 列）');
    await sleep(3000);
    const logReqs = ctx.net.since(t0, 'git_log');
    check(logReqs.length === 1, `隱藏的 Git Graph 分頁不會自動載入下一批：3 秒後 /log 請求恰 1 次（實際 ${logReqs.length}：${JSON.stringify(logReqs.map((r) => r.url))}）`);
    const rowsHidden = await ctx.cdp.run((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length, root.root_id);
    check(rowsHidden === 200, `隱藏期間仍只有 200 列（實際 ${rowsHidden}）`);

    need(await ctx.cdp.clickEl((rid) => window.__gc.graphTab(rid), [root.root_id], 'Git Graph 分頁'), '切回 Git Graph 分頁');
    let finalRows = 0;
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
      await ctx.cdp.run((rid) => {
        const sc = window.__gc.graphScroller(window.__gc.graphPanelOf(rid));
        if (sc) sc.scrollTop = sc.scrollHeight;
      }, root.root_id);
      await sleep(400);
      finalRows = await ctx.cdp.run((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length, root.root_id);
      if (finalRows === 264) break;
    }
    check(finalRows === 264, `切回並捲到底後仍能繼續載入到 264 列（實際 ${finalRows}）`);
  });
}

// M2：進入「比較」詳情時 detailData 還是上一個 commit 詳情（形狀不同）或 null，載入中標頭讀到
// undefined／拋 TypeError。GIVEN 已選 A 並載入詳情 WHEN Ctrl+點 B THEN 標頭（含載入中的每個瞬間）不含
// undefined；GIVEN C 詳情還在載入 WHEN 立刻 Ctrl+點 D THEN 無 pageerror、面板不空白。
async function segCompareHeaderNoUndefined() {
  await withCockpit('compare-header-no-undefined', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await openChanges(ctx);
    await openGraphTab(ctx, root);
    const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
    need(!!loaded, '前置：第一批 200 列載入完成');
    const oids = await ctx.cdp.run((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).slice(0, 6).map((r) => r.getAttribute('data-oid')), root.root_id);
    need(oids.length === 6 && oids.every((o) => /^[0-9a-f]{40}$/.test(o)), `前置：取得前 6 列的 oid（實際 ${JSON.stringify(oids)}）`);
    const [oidA, oidB, oidC, oidD] = oids;

    // 情境一：A 詳情已載入，Ctrl+點 B；MutationObserver 記錄詳情面板每一個瞬間的文字。
    need(await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, oidA], 'A 列'), '點 A 列');
    const aReady = await ctx.cdp.poll(
      (rid) => {
        const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
        return !!t && !t.includes('正在讀取');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!aReady, 'A 的詳情載入完成');
    const exc0 = ctx.console.filter((e) => e.level === 'exception').length;
    await ctx.cdp.run(
      (rid, oid) => {
        const panel = window.__gc.graphPanelOf(rid);
        window.__m2Snaps = [];
        const snap = () => {
          const t = window.__gc.commitDetailText(panel);
          if (t !== null) window.__m2Snaps.push(t);
        };
        window.__m2Obs = new MutationObserver(snap);
        window.__m2Obs.observe(panel, { childList: true, subtree: true, characterData: true });
        const row = window.__gc.graphRowByOid(panel, oid);
        row.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, ctrlKey: true }));
        snap();
        return true;
      },
      root.root_id,
      oidB
    );
    const cmpReady = await ctx.cdp.poll(
      (rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const t = window.__gc.commitDetailText(p);
        return !!t && !!window.__gc.commitDetailButton(p, '直接比較') && !t.includes('正在讀取');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!cmpReady, 'Ctrl+點 B 後詳情成為「比較」畫面且載入完成');
    const snaps = await ctx.cdp.run(() => {
      window.__m2Obs.disconnect();
      return window.__m2Snaps;
    });
    const bad = snaps.filter((t) => t.includes('undefined'));
    check(snaps.length > 0 && bad.length === 0, `比較詳情的每個瞬間（共 ${snaps.length} 個快照）標頭都不含 undefined（含 undefined 的快照：${JSON.stringify(bad.slice(0, 2).map((t) => t.slice(0, 80)))}）`);
    const finalHead = await ctx.cdp.run((rid) => window.__gc.commitDetailText(window.__gc.graphPanelOf(rid)), root.root_id);
    check(finalHead.includes(oidA.slice(0, 7)) && finalHead.includes(oidB.slice(0, 7)), `比較標頭含 A、B 短 hash（實際「${finalHead.slice(0, 100)}」）`);

    // 情境二：C 詳情還在載入（detailData 為 null）時立刻 Ctrl+點 D，須無例外且詳情面板有內容。
    await ctx.cdp.run(
      (rid, c, d) => {
        const panel = window.__gc.graphPanelOf(rid);
        window.__gc.graphRowByOid(panel, c).dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
        window.__gc.graphRowByOid(panel, d).dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, ctrlKey: true }));
        return true;
      },
      root.root_id,
      oidC,
      oidD
    );
    const cdReady = await ctx.cdp.poll(
      (rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const t = window.__gc.commitDetailText(p);
        return !!t && !!window.__gc.commitDetailButton(p, '直接比較') && !t.includes('正在讀取');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    check(!!cdReady, 'C 詳情載入中就 Ctrl+點 D：詳情面板仍顯示「比較」畫面（未卡空白）');
    const exc1 = ctx.console.filter((e) => e.level === 'exception');
    check(exc1.length === exc0, `整個過程沒有 pageerror（新增 ${exc1.length - exc0} 筆：${JSON.stringify(exc1.slice(exc0).map((e) => (e.text || '').slice(0, 120)))}）`);
  });
}

// M3：詳情檔案列整列是 role=button 並處理 Enter／Space；內嵌的「看此版本」按鈕按 Enter 事件會冒泡被整列攔走，
// 開成 diff 分頁。GIVEN 詳情列出 history/counter.txt WHEN 鍵盤聚焦其「看此版本」按 Enter THEN 開的是某版本
// 分頁、沒有 diff 分頁。
async function segViewVersionButtonKeyboard() {
  await withCockpit('view-version-keyboard', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    await openChanges(ctx);
    await openGraphTab(ctx, root);
    const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
    need(!!loaded, '前置：第一批 200 列載入完成');
    const rowOid = await ctx.cdp.run((rid) => {
      const r = window.__gc.graphRowBySubject(window.__gc.graphPanelOf(rid), 'commit #210');
      return r ? r.getAttribute('data-oid') : null;
    }, root.root_id);
    need(!!rowOid && /^[0-9a-f]{40}$/.test(rowOid), `前置：找到 commit #210 並取得 hash（實際 ${rowOid}）`);
    need(await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, rowOid], 'commit #210 列'), '點 commit #210 列');
    const ready = await ctx.cdp.poll(
      (rid) => {
        const p = window.__gc.graphPanelOf(rid);
        const t = window.__gc.commitDetailText(p);
        return !!t && !t.includes('正在讀取') && !!window.__gc.commitDetailFileRow(p, 'history/counter.txt');
      },
      [root.root_id],
      UI_TIMEOUT_MS
    );
    need(!!ready, '前置：詳情載入完成並列出 history/counter.txt');
    const focused = await ctx.cdp.run((rid) => {
      const row = window.__gc.commitDetailFileRow(window.__gc.graphPanelOf(rid), 'history/counter.txt');
      const btn = row ? Array.from(row.querySelectorAll('button')).find((b) => window.__gc.txt(b) === '看此版本') : null;
      if (!btn) return false;
      btn.focus();
      return document.activeElement === btn;
    }, root.root_id);
    need(focused, '前置：鍵盤聚焦 history/counter.txt 的「看此版本」按鈕');
    const keyBase = { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 };
    await ctx.cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', text: '\r', ...keyBase });
    await ctx.cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', ...keyBase });
    const revOpened = await ctx.cdp.poll(
      (p, rev) => {
        const t = window.__gc.revTab(p, rev);
        return !!t && t.getAttribute('aria-selected') === 'true';
      },
      ['history/counter.txt', rowOid],
      UI_TIMEOUT_MS
    );
    check(!!revOpened, `「看此版本」按 Enter 開的是某版本分頁（history/counter.txt @ ${rowOid.slice(0, 7)}）`);
    const diffTabs = await ctx.cdp.run(() => window.__gc.reviewTabs().filter((t) => t.getAttribute('data-diff-path') === 'history/counter.txt').length);
    check(diffTabs === 0, `沒有開出 history/counter.txt 的 diff 分頁（實際 ${diffTabs} 個）`);
  });
}

// ui-fixes task 4.8（spec git-review「commit 詳情與比較」焦點段落、design D7）：詳情區因重新載入而重建時，
// 焦點必須回到重建後代表同一個對象的元素上，不得落到 body；焦點外框依最近一次輸入方式決定。
// 兩個 commit（HEAD 與 feature/logging 的 tip）的比較詳情已顯示後，切換「自分岔點起」會重建詳情兩次
// （載入中、載入完成），焦點兩次都必須留在對應的切換按鈕上。
async function openCompareDetailForFocus(ctx) {
  const root = await rootInfo(ctx, PANE_REVIEW);
  const status = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/status`);
  need(status.status === 200 && status.body && status.body.branch, `前置：狀態端點回 200（實際 ${status.status}）`);
  const eOid = status.body.branch.oid;
  const refs = await apiJson(ctx, `/api/git/${RUNTIME}/${root.root_id}/refs`);
  const fRef = refs.body && Array.isArray(refs.body.refs) ? refs.body.refs.find((r) => r.kind === 'branch' && r.short === 'feature/logging') : null;
  need(!!fRef && /^[0-9a-f]{40}$/.test(fRef.oid), `前置：找到 feature/logging 分支與其 oid（實際 ${JSON.stringify(fRef)}）`);
  const fOid = fRef.oid;
  await openChanges(ctx);
  await openGraphTab(ctx, root);
  const loaded = await ctx.cdp.poll((rid) => window.__gc.graphRows(window.__gc.graphPanelOf(rid)).length >= 200, [root.root_id], UI_TIMEOUT_MS);
  need(!!loaded, '前置：第一批 200 列載入完成（E、F 都在其中）');
  need(await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, eOid], 'commit E（HEAD）列'), '點 commit E（HEAD）列');
  const eReady = await ctx.cdp.poll(
    (rid) => {
      const t = window.__gc.commitDetailText(window.__gc.graphPanelOf(rid));
      return !!t && !t.includes('正在讀取');
    },
    [root.root_id],
    UI_TIMEOUT_MS
  );
  need(!!eReady, 'commit E 的詳情載入完成');
  need(await ctx.cdp.clickEl((rid) => window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '選為比較基準'), [root.root_id], '「選為比較基準」按鈕'), '點「選為比較基準」按鈕');
  need(await ctx.cdp.clickEl((rid, oid) => window.__gc.graphRowByOid(window.__gc.graphPanelOf(rid), oid), [root.root_id, fOid], 'commit F 列'), '點 commit F 列');
  const compareReady = await ctx.cdp.poll(
    (rid) => {
      const p = window.__gc.graphPanelOf(rid);
      const t = window.__gc.commitDetailText(p);
      return !!t && t.includes('比較') && !!window.__gc.commitDetailButton(p, '直接比較') && !t.includes('正在讀取');
    },
    [root.root_id],
    UI_TIMEOUT_MS
  );
  need(!!compareReady, '前置：比較詳情已顯示且載入完成');
  return root;
}

// 目前焦點：是否在詳情區內、是否為「自分岔點起」按鈕、是否匹配 :focus-visible、舊節點是否已脫離 DOM。
const FOCUS_PROBE = (rid) => {
  const a = document.activeElement;
  const p = window.__gc.graphPanelOf(rid);
  const wrap = window.__gc.commitDetailWrap(p);
  const forkBtn = window.__gc.commitDetailButton(p, '自分岔點起');
  return {
    isBody: a === document.body,
    inDetail: !!a && !!wrap && wrap.contains(a),
    isForkBtn: !!a && a === forkBtn,
    text: a ? (a.textContent || '').trim().slice(0, 20) : null,
    focusVisible: !!a && a.matches(':focus-visible'),
    oldConnected: window.__oldFork ? window.__oldFork.isConnected : null,
    pressed: forkBtn ? forkBtn.getAttribute('aria-pressed') : null,
  };
};

const FORK_DONE = (rid) => {
  const p = window.__gc.graphPanelOf(rid);
  const toggle = window.__gc.commitDetailToggle(p, '自分岔點起');
  return !!toggle && toggle.pressed === 'true' && !window.__gc.commitDetailText(p).includes('正在讀取');
};

// GIVEN 兩個 commit 的比較詳情已顯示，以鍵盤 Tab 把焦點移到「自分岔點起」切換按鈕上（外框可見）
// WHEN 按 Enter 切換，比較結果載入完成、詳情區重建 THEN 焦點在重建後代表同一個操作的切換按鈕上，
// 不在 body 上，且該按鈕匹配 :focus-visible。
async function segDetailRebuildFocusKeyboard() {
  await withCockpit('detail-rebuild-focus-keyboard', {}, async (ctx) => {
    const root = await openCompareDetailForFocus(ctx);
    // 把焦點放到「直接比較」按鈕（程式聚焦），再以真實 Tab 移到下一個可聚焦元素＝「自分岔點起」。
    await ctx.cdp.run((rid) => window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '直接比較').focus(), root.root_id);
    await ctx.cdp.press('Tab');
    const before = await ctx.cdp.run(FOCUS_PROBE, root.root_id);
    need(before.isForkBtn, `前置：Tab 之後焦點在「自分岔點起」按鈕（實際 ${JSON.stringify(before)}）`);
    check(before.focusVisible === true, `前置：Tab 到按鈕時外框可見（:focus-visible；實際 ${JSON.stringify(before)}）`);
    await ctx.cdp.run((rid) => {
      window.__oldFork = window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '自分岔點起');
    }, root.root_id);
    await ctx.cdp.press('Enter');
    const done = await ctx.cdp.poll(FORK_DONE, [root.root_id], UI_TIMEOUT_MS);
    need(!!done, '按 Enter 後「自分岔點起」查詢完成');
    const after = await ctx.cdp.run(FOCUS_PROBE, root.root_id);
    check(after.oldConnected === false, `詳情確實重建了（舊的「自分岔點起」節點已脫離 DOM；實際 ${JSON.stringify(after)}）`);
    check(!after.isBody, `重建後焦點不在 body 上（實際 ${JSON.stringify(after)}）`);
    check(after.isForkBtn, `重建後焦點在代表同一個操作的「自分岔點起」切換按鈕上（實際 ${JSON.stringify(after)}）`);
    check(after.focusVisible === true, `鍵盤觸發：重建後的按鈕匹配 :focus-visible、外框照常呈現（實際 ${JSON.stringify(after)}）`);
  });
}

// GIVEN 以滑鼠點了詳情中的「自分岔點起」切換按鈕 WHEN 詳情因此重建 THEN 焦點在重建後的對應切換按鈕上，
// 不在 body 上，且該按鈕不匹配 :focus-visible、沒有焦點外框。（真實滑鼠：CDP Input.dispatchMouseEvent。）
async function segDetailRebuildFocusMouse() {
  await withCockpit('detail-rebuild-focus-mouse', {}, async (ctx) => {
    const root = await openCompareDetailForFocus(ctx);
    await ctx.cdp.run((rid) => {
      window.__oldFork = window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '自分岔點起');
    }, root.root_id);
    need(await ctx.cdp.clickEl((rid) => window.__gc.commitDetailButton(window.__gc.graphPanelOf(rid), '自分岔點起'), [root.root_id], '「自分岔點起」按鈕'), '以真實滑鼠點「自分岔點起」按鈕');
    const done = await ctx.cdp.poll(FORK_DONE, [root.root_id], UI_TIMEOUT_MS);
    need(!!done, '點擊後「自分岔點起」查詢完成');
    const after = await ctx.cdp.run(FOCUS_PROBE, root.root_id);
    check(after.oldConnected === false, `詳情確實重建了（舊的「自分岔點起」節點已脫離 DOM；實際 ${JSON.stringify(after)}）`);
    check(!after.isBody, `重建後焦點不在 body 上（實際 ${JSON.stringify(after)}）`);
    check(after.isForkBtn, `重建後焦點在代表同一個操作的「自分岔點起」切換按鈕上（實際 ${JSON.stringify(after)}）`);
    check(after.focusVisible === false, `滑鼠觸發：重建後的按鈕不匹配 :focus-visible、沒有焦點外框（實際 ${JSON.stringify(after)}）`);
  });
}

function isNonEmptyStringLike(value) {
  return typeof value === 'string' && value.length > 0;
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

const SEGMENTS = [
  { code: 'self/鷹架', fn: segSelfScaffold, self: true },
  { code: 'self/段落代號', fn: segSelfSegmentArg, self: true },
  { code: 'git-review/顯示變更並開啟 diff', fn: segShowChangesAndOpenDiff },
  { code: 'git-review/新變更自動出現', fn: segNewChangeAppears },
  { code: 'git-review/不是目前分頁時不讀取', fn: segNoStatusWhenNotCurrent },
  { code: 'git-review/重畫不影響變更分頁', fn: segRepaintKeepsChanges },
  { code: 'git-review/左右並排呈現', fn: segSideBySideRendering },
  { code: 'git-review/合併衝突開 diff', fn: segMergeConflictOpenDiff },
  { code: 'git-review/刪除檔的工具列停用', fn: segDeletedFileToolbarDisabled },
  { code: 'git-review/改檔後更新', fn: segUpdateAfterChange },
  { code: 'git-review/窄視窗不橫向捲動', fn: segNoHorizontalScroll },
  { code: 'git-review/開啟並分批載入', fn: segOpenAndBatchLoad },
  { code: 'git-review/搜尋跳轉', fn: segSearchJump },
  { code: 'git-review/分支變更提示', fn: segRefsChangedBanner },
  { code: 'git-review/搜尋命中後背景載入不拉動捲動', fn: segSearchBackgroundLoadNoScroll },
  { code: 'git-review/有非 commit tag 時不誤報分支變更', fn: segNonCommitTagNoFalseBanner },
  { code: 'git-review/重畫不影響 Git Graph', fn: segRepaintKeepsGraph },
  { code: 'git-review/看 commit 的變更並開 diff', fn: segCommitDetailOpenDiff },
  { code: 'git-review/比較兩個 commit', fn: segCompareCommits },
  { code: 'git-review/commit 詳情檔案清單被截斷', fn: segCommitDetailTruncationNote },
  { code: 'git-review/兩個 commit 比較的檔案清單被截斷', fn: segCompareTruncationNote },
  { code: 'git-review/詳情重建後焦點留在對應元素', fn: segDetailRebuildFocusKeyboard },
  { code: 'git-review/滑鼠觸發的詳情重建不呈現焦點外框', fn: segDetailRebuildFocusMouse },
  { code: 'git-review/複製 hash', fn: segCopyHash },
  { code: 'git-review/看舊版規格', fn: segOldVersion },
  { code: 'git-review/commit 版本不輪詢', fn: segCommitVersionNoPoll },
  { code: 'file-review/還原 git 分頁與變更分頁', fn: segRestoreGitAndChangesTabs },
  { code: 'git-review/隱藏的 Git Graph 不自動載入', fn: segHiddenGraphNoAutoLoad },
  { code: 'git-review/比較詳情標頭不含 undefined', fn: segCompareHeaderNoUndefined },
  { code: 'git-review/看此版本按鈕鍵盤操作', fn: segViewVersionButtonKeyboard },
];

async function main() {
  const known = SEGMENTS.map((s) => s.code);
  const parsed = POSITIONAL.length > 1 ? { ok: false, message: `只接受一個段落參數（逗號分隔），實際 ${POSITIONAL.length} 個：${JSON.stringify(POSITIONAL)}` } : parseSegmentArg(SEGMENT_ARG, known);
  if (!parsed.ok) {
    console.error(`FAIL ${parsed.message}`);
    console.log('RESULT: FAIL (段落代號)');
    process.exitCode = 2;
    return;
  }
  const only = parsed.codes;
  if (!fs.existsSync(CHROME)) throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  const others = runningUiPreviewPids();
  if (others.length > 0 || isPortListening(7770)) {
    console.error(`FAIL 已有 ui_preview.exe 在執行（PID ${JSON.stringify(others)}）或 127.0.0.1:7770 有人 LISTEN——與其他驗收腳本並行會互相干擾，本腳本不動別人的行程，請先確認後再跑`);
    console.log('RESULT: FAIL (環境)');
    process.exitCode = 2;
    return;
  }
  const leftovers = ourChromePids();
  if (leftovers.length > 0) {
    log(`清掉上一次本腳本殘留的 headless Chrome（user-data-dir 含 ${CHROME_UDD_PREFIX}）：${JSON.stringify(leftovers)}`);
    for (const pid of leftovers) spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], { encoding: 'utf8' });
  }
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
  check(!isPortListening(7770), '結束後 port 7770 沒有 LISTENING 的行程');
  finalSweep();
  check(runningUiPreviewPids().length === 0, '結束後沒有殘留的 ui_preview.exe');
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
