// ui-fixes 驗收（畫面與操作修補）：新斷言集中在這支腳本，各 task 擴充。
// 各段代號與對應 scenario 見同目錄 ui-fixes-check.md。
//
// 目前涵蓋 task 4.2（spec cockpit-dashboard「畫面整頁重畫」焦點外框三個 scenario，design D1）：
//   F1 滑鼠點 pane 列、F2 滑鼠點左欄專案、F3 滑鼠點 task 按鈕：新載入頁面後的第一次點擊，
//      重畫還原焦點後不得匹配 :focus-visible（含其後約 1 秒內的背景重畫）。
//   F4 取消選取（滑鼠點「取消選取」）後焦點交接到 pane 列或面板，同樣不出框。
//   F5 鍵盤 Tab 到「推進」按鈕後按 Enter：重畫後的對應按鈕仍匹配 :focus-visible。
//   F6 滑鼠點 pane 列之後按 Tab：焦點外框照常出現。
//   F7 滑鼠點 pane 列之後按 Alt、Ctrl（修飾鍵不算鍵盤輸入，修正波 1 F-I1）：背景重畫後外框仍不出現。
//   F8 鍵盤 Tab 到「推進」按鈕（外框可見）後，對不移動焦點的位置送真實滑鼠 pointerdown（修正波 1 F-M1）：
//      背景重畫後焦點仍在同一按鈕、外框仍在。
// 與 task 4.3（spec cockpit-dashboard「畫面整頁重畫」通道斷線相關 scenario，design D2）：
//   D1 通道斷線（以 window.onChannel('disconnected') 模擬，同 visual-check.js CH1 的手法）時，逐區以 getComputedStyle
//      比對 --text-dim：runtime 卡連線狀態（含「最後已知」手足節點）、pane／workspace／tab 的 agent 狀態、task 節點
//      （色條、狀態文字、running／failed／blocked 外框與柔光）、列首「未宣告 task」提示、左欄 project 狀態計數；
//      按鈕不轉暗；恢復 connected 後各處還原。
//   D2 斷線中點 pane 列觸發整頁重畫後仍為最後已知；#app 的 data-channel-state 被移除後 paint() 寫回。
//   D3 真的停掉 ui_preview 造成通道斷線、再重啟恢復（不靠模擬）。
//   D4 `--screenshots`：斷線狀態 1536／1100／700 寬截圖，存 ui-fixes-disconnected-<寬>.png（設計審核素材）。
// 與 task 4.4（spec cockpit-dashboard「Factory Floor」「畫面操作」覆蓋斷線相關 scenario，design D3）：
//   B1 覆蓋造成的斷線（cockpit/ovr，source=override）：綁定摘要有「runtime 未連線」與「改綁」徽章，列首同時有
//      「改綁」按鈕與「取消改綁」按鈕。
//   B2 自動綁定的斷線（cockpit/docs、p/tests，source=auto）：沒有徽章、沒有「取消改綁」，但有「改綁」按鈕。
//   B3 停在該狀態整頁重畫（window.repaint()、點 pane 列、其後背景重畫）後仍正確。
//   B4 真滑鼠按 ovr 的「取消改綁」：服務恰好收到 DELETE /api/projects/cockpit/workstreams/ovr/override。
//
// 用 raw CDP 的 Input.dispatchMouseEvent／Input.dispatchKeyEvent 產生真實輸入（不是 el.click()／
// el.focus()），因為 :focus-visible 取決於瀏覽器對「最近一次真實輸入」的判斷。
//
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-10-01/ui-fixes-check.js [--screenshots]
// 清理：只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID），埠被占用就往上找空埠。
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function pidStillRunning(pid) {
  const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
  return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
}

function killTree(child, label) {
  if (!child || child.exitCode !== null) return;
  spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
  check(!pidStillRunning(child.pid), `${label} PID ${child.pid} 已終止（tasklist 查無此 PID）`);
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

class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        this.pending.get(m.id)(m);
        this.pending.delete(m.id);
      }
    };
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((res) => {
      this.pending.set(id, res);
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  async eval(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true });
    if (r.result && r.result.exceptionDetails) {
      throw new Error(`頁面內例外：${JSON.stringify(r.result.exceptionDetails)}`);
    }
    return r.result && r.result.result ? r.result.result.value : undefined;
  }
  async waitFor(expression, timeoutMs, label) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      if (await this.eval(expression)) {
        check(true, label);
        return true;
      }
      await sleep(50);
    }
    check(false, `逾時（${timeoutMs} ms）：${label}`);
    return false;
  }
  // 以真的滑鼠事件點 selector 指到的元素中心。
  async click(selector) {
    const rect = await this.eval(
      `(() => { const n = document.querySelector(${JSON.stringify(selector)});
        if (!n) return null; n.scrollIntoView({block: 'center'});
        const r = n.getBoundingClientRect(); return {x: r.left + r.width / 2, y: r.top + r.height / 2}; })()`
    );
    if (!rect) {
      check(false, `找不到可點的元素：${selector}`);
      return false;
    }
    const base = { x: rect.x, y: rect.y, button: 'left', clickCount: 1 };
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x, y: rect.y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
    return true;
  }
  // 真實按鍵（keyDown＋keyUp）。
  async press(name) {
    const keys = {
      Tab: { key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 },
      Enter: { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' },
      // 修飾鍵本身（沒有 text，用 rawKeyDown；modifiers 位元：Alt=1、Ctrl=2、Meta=4、Shift=8）。
      Alt: { key: 'Alt', code: 'AltLeft', windowsVirtualKeyCode: 18, modifiers: 1, raw: true },
      Control: { key: 'Control', code: 'ControlLeft', windowsVirtualKeyCode: 17, modifiers: 2, raw: true },
    };
    const { raw, ...k } = keys[name];
    await this.send('Input.dispatchKeyEvent', { type: raw ? 'rawKeyDown' : 'keyDown', ...k });
    const { text, ...up } = k;
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', ...up });
  }
}

async function startChrome(cdpPort, url, label) {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), `cockpit-chrome-${label}-`));
  const chrome = spawn(
    CHROME,
    [
      '--headless=new', '--lang=zh-TW',
      '--disable-gpu',
      '--no-first-run',
      `--remote-debugging-port=${cdpPort}`,
      '--remote-allow-origins=*',
      `--user-data-dir=${udd}`,
      '--window-size=1400,1600',
      url,
    ],
    { stdio: 'ignore', windowsHide: true }
  );
  chrome.on('error', (e) => check(false, `${label} spawn error：${e.message}`));
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
  if (!page) throw new Error(`${label}: page target not found`);
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = rej;
  });
  return { chrome, udd, ws, cdp: new CDP(ws) };
}

async function stopChrome(handle, label) {
  if (!handle) return;
  try {
    handle.ws.close();
  } catch {
    // 已斷線。
  }
  killTree(handle.chrome, label);
  await sleep(500);
  try {
    fs.rmSync(handle.udd, { recursive: true, force: true });
  } catch (e) {
    check(false, `清理暫存目錄失敗：${e.message}`);
  }
}

// ---------------------------------------------------------------------------
// 共用讀取
// ---------------------------------------------------------------------------

// 投影 version 讀 #version 的 data-state-version（2026-10-05 起文字改為程式版本，例如 v0.1.2）。
const versionOf = "Number(document.getElementById('version').getAttribute('data-state-version'))";

// 目前焦點元素：身分（dataset 的 JSON）、是否匹配 :focus-visible。
const readFocus = `(() => {
  const a = document.activeElement;
  if (!a || a === document.body) return { body: true };
  return {
    body: false,
    tag: a.tagName,
    cls: a.className,
    identity: JSON.stringify(a.dataset || {}),
    focusVisible: a.matches(':focus-visible'),
  };
})()`;

// 以 selector 找到的第一個元素的身分（dataset 的 JSON）。
const identityOf = (selector) =>
  `(() => { const n = document.querySelector(${JSON.stringify(selector)}); return n ? JSON.stringify(n.dataset) : null; })()`;

// 新載入頁面（第一次點擊最容易漏：此時還沒有任何 pointerdown／keydown 記錄）。
async function freshLoad(cdp, url) {
  await cdp.eval('window.__old = true; true');
  await cdp.send('Page.navigate', { url });
  await cdp.waitFor(
    "window.__old === undefined && document.querySelectorAll('.task-node').length >= 9 && document.querySelectorAll('.pane-row[data-action=\"select-pane\"]').length >= 1",
    8000,
    '重新載入頁面並畫出 task 節點與 pane 列'
  );
  // 等到第一次重畫之後，讓頁面進入穩定的 100 ms 推送節奏。
  await sleep(300);
}

// ---------------------------------------------------------------------------
// F1–F3 滑鼠第一次點擊，F4 取消選取
// ---------------------------------------------------------------------------

async function mouseCase(cdp, url, code, name, selector) {
  log(`--- ${code} 滑鼠點${name}（新載入頁面後的第一次點擊）---`);
  await freshLoad(cdp, url);
  const want = await cdp.eval(identityOf(selector));
  check(want !== null, `${code}：找到目標元素 ${selector}`);
  if (want === null) return;
  await cdp.eval(`window.__m = document.querySelector(${JSON.stringify(selector)}); true`);
  const v0 = await cdp.eval(versionOf);
  await cdp.click(selector);

  // 點擊剛結束（同步重畫已發生）。
  const f1 = await cdp.eval(readFocus);
  check(f1.body === false && f1.identity === want, `${code}：點擊後焦點在代表同一對象的元素上（期望 ${want}；實際 ${JSON.stringify(f1)}）`);
  check(f1.focusVisible === false, `${code}：點擊後（重畫當下）焦點元素不匹配 :focus-visible（實際 ${f1.focusVisible}）`);

  // 其後約 1 秒的背景重畫。
  await sleep(1000);
  const v1 = await cdp.eval(versionOf);
  check(v1 >= v0 + 3, `${code}：這 1 秒內有收到至少 3 份新投影、重畫數次（version ${v0} -> ${v1}）`);
  check(await cdp.eval('window.__m !== document.querySelector(' + JSON.stringify(selector) + ')'), `${code}：目標元素已被重畫換成新節點`);
  const f2 = await cdp.eval(readFocus);
  check(f2.body === false && f2.identity === want, `${code}：1 秒後焦點仍在代表同一對象的新元素上（實際 ${JSON.stringify(f2)}）`);
  check(f2.focusVisible === false, `${code}：1 秒後（背景重畫之後）焦點元素不匹配 :focus-visible（實際 ${f2.focusVisible}）`);
}

async function deselectCase(cdp, url) {
  log('--- F4 滑鼠點「取消選取」（焦點交接）---');
  await freshLoad(cdp, url);
  await cdp.click('.pane-row[data-action="select-pane"]');
  await cdp.waitFor("document.getElementById('output').classList.contains('is-open')", 3000, 'F4：面板已開啟');
  await sleep(300);
  await cdp.click('.output-close');
  await cdp.waitFor("!document.getElementById('output').classList.contains('is-open')", 3000, 'F4：面板已收起');
  await sleep(500);
  const f = await cdp.eval(readFocus);
  check(f.body === false, `F4：取消選取後焦點沒有掉到 body（實際 ${JSON.stringify(f)}）`);
  check(f.focusVisible === false, `F4：取消選取後交接到 pane 列或面板的焦點不匹配 :focus-visible（實際 ${JSON.stringify(f)}）`);
}

// ---------------------------------------------------------------------------
// F5 鍵盤 Enter 觸發重畫、F6 滑鼠之後 Tab
// ---------------------------------------------------------------------------

async function tabUntil(cdp, predicateExpr, max = 400) {
  for (let i = 0; i < max; i++) {
    await cdp.press('Tab');
    if (await cdp.eval(`(() => { const a = document.activeElement; return !!a && (${predicateExpr})(a); })()`)) return i + 1;
  }
  return 0;
}

async function keyboardCase(cdp, url, requests) {
  log('--- F5 鍵盤 Tab 到「推進」後按 Enter ---');
  await freshLoad(cdp, url);
  const hops = await tabUntil(cdp, "(a) => a.dataset && a.dataset.action === 'advance'");
  check(hops > 0, `F5：以 Tab 把焦點移到某 task 的「推進」按鈕（Tab ${hops} 次）`);
  if (hops === 0) return;
  const before = await cdp.eval(readFocus);
  check(before.focusVisible === true, `F5：Tab 到按鈕時外框可見（:focus-visible；實際 ${JSON.stringify(before)}）`);
  requests.length = 0;
  const v0 = await cdp.eval(versionOf);
  await cdp.press('Enter');
  await sleep(1000);
  check(
    requests.length === 1 && requests[0].method === 'POST' && /\/tasks\/[^/]+\/advance$/.test(requests[0].path),
    `F5：Enter 之後服務收到恰好一個 advance 請求（實際 ${JSON.stringify(requests)}）`
  );
  const v1 = await cdp.eval(versionOf);
  check(v1 >= v0 + 3, `F5：Enter 之後收到至少 3 份新投影、重畫數次（version ${v0} -> ${v1}）`);
  const after = await cdp.eval(readFocus);
  // spec：「代表同一個 task 的按鈕（或該 task 在新畫面中仍可互動的對應按鈕）」。ui_preview 不改
  // fixture，所以「推進」仍在，身分應完全相同。
  check(after.body === false && after.identity === before.identity, `F5：重畫後焦點仍在同一個 task 的「推進」按鈕上（期望 ${before.identity}；實際 ${JSON.stringify(after)}）`);
  check(after.focusVisible === true, `F5：鍵盤觸發的重畫後焦點元素匹配 :focus-visible（外框保留；實際 ${after.focusVisible}）`);
}

async function mouseThenTabCase(cdp, url) {
  log('--- F6 滑鼠點 pane 列之後按 Tab ---');
  await freshLoad(cdp, url);
  const selector = '.pane-row[data-action="select-pane"]';
  const want = await cdp.eval(identityOf(selector));
  await cdp.click(selector);
  await sleep(500);
  const f0 = await cdp.eval(readFocus);
  check(f0.identity === want && f0.focusVisible === false, `F6：點擊後焦點在 pane 列且外框不可見（實際 ${JSON.stringify(f0)}）`);
  await cdp.press('Tab');
  const f1 = await cdp.eval(readFocus);
  check(f1.body === false && f1.identity !== want, `F6：Tab 把焦點移到另一個元素（實際 ${JSON.stringify(f1)}）`);
  check(f1.focusVisible === true, `F6：Tab 之後焦點元素匹配 :focus-visible、外框照常出現（實際 ${f1.focusVisible}）`);
  // 其後的背景重畫不得把外框洗掉（焦點若在 #app 內，會被重畫還原）。
  await sleep(600);
  const f2 = await cdp.eval(readFocus);
  check(f2.body === false && f2.focusVisible === true, `F6：Tab 之後 600 ms 的背景重畫之後外框仍在（實際 ${JSON.stringify(f2)}）`);
}

// F7：修飾鍵（Alt／Ctrl）不是「鍵盤輸入」：Chrome 原生的 :focus-visible 判定忽略它們，
// 所以滑鼠操作後單按修飾鍵不能把最後輸入方式記成 keyboard（否則下一次背景重畫外框又出現）。
async function modifierKeyCase(cdp, url) {
  log('--- F7 滑鼠點 pane 列之後按 Alt／Ctrl ---');
  const selector = '.pane-row[data-action="select-pane"]';
  for (const key of ['Alt', 'Control']) {
    await freshLoad(cdp, url);
    const want = await cdp.eval(identityOf(selector));
    await cdp.click(selector);
    await sleep(500);
    const f0 = await cdp.eval(readFocus);
    check(f0.identity === want && f0.focusVisible === false, `F7（${key}）：點擊後焦點在 pane 列且外框不可見（實際 ${JSON.stringify(f0)}）`);
    const v0 = await cdp.eval(versionOf);
    await cdp.press(key);
    await sleep(800);
    const v1 = await cdp.eval(versionOf);
    check(v1 >= v0 + 3, `F7（${key}）：按鍵之後收到至少 3 份新投影、重畫數次（version ${v0} -> ${v1}）`);
    const f1 = await cdp.eval(readFocus);
    check(f1.body === false && f1.identity === want, `F7（${key}）：背景重畫後焦點仍在同一個 pane 列（實際 ${JSON.stringify(f1)}）`);
    check(f1.focusVisible === false, `F7（${key}）：按 ${key} 並歷經背景重畫後焦點元素仍不匹配 :focus-visible（實際 ${f1.focusVisible}）`);
  }
}

// F8：鍵盤 Tab 到按鈕（外框可見）之後，對「不移動焦點」的位置做滑鼠操作（按住捲軸之類），
// 重畫不得把外框洗掉。優先用真的捲軸（有出現 classic scrollbar 的可捲容器，按在捲軸上）；
// 找不到就改按一個自己放進頁面、mousedown 預設動作被取消（不移動焦點）的非可聚焦元素——
// 兩者都是 CDP 真實滑鼠事件，會觸發 document 上的 pointerdown。
const findScrollbarSpot = `(() => {
  for (const n of document.querySelectorAll('*')) {
    const cs = getComputedStyle(n);
    const bw = (parseFloat(cs.borderLeftWidth) || 0) + (parseFloat(cs.borderRightWidth) || 0);
    const sw = n.offsetWidth - n.clientWidth - bw;
    if (sw >= 6 && n.scrollHeight > n.clientHeight && /(auto|scroll)/.test(cs.overflowY)) {
      n.scrollIntoView({ block: 'nearest' });
      const r = n.getBoundingClientRect();
      if (r.width > 0 && r.height > 40 && r.top >= 0 && r.bottom <= innerHeight) {
        return { x: r.right - (parseFloat(cs.borderRightWidth) || 0) - sw / 2, y: r.top + r.height / 2, how: 'scrollbar:' + (n.className || n.tagName) };
      }
    }
  }
  return null;
})()`;

async function pointerWithoutFocusMoveCase(cdp, url) {
  log('--- F8 鍵盤 Tab 到「推進」後，對不移動焦點的位置送滑鼠 pointerdown ---');
  await freshLoad(cdp, url);
  const hops = await tabUntil(cdp, "(a) => a.dataset && a.dataset.action === 'advance'");
  check(hops > 0, `F8：以 Tab 把焦點移到某 task 的「推進」按鈕（Tab ${hops} 次）`);
  if (hops === 0) return;
  const before = await cdp.eval(readFocus);
  check(before.focusVisible === true, `F8：Tab 到按鈕時外框可見（實際 ${JSON.stringify(before)}）`);

  let spot = await cdp.eval(findScrollbarSpot);
  if (spot === null) {
    spot = await cdp.eval(`(() => {
      const d = document.createElement('div');
      d.id = '__nofocus';
      d.style.cssText = 'position:fixed;left:2px;bottom:2px;width:40px;height:40px;z-index:99999;';
      d.addEventListener('mousedown', (e) => e.preventDefault());
      document.body.appendChild(d);
      const r = d.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2, how: 'mousedown 預設動作被取消的非可聚焦元素' };
    })()`);
  }
  log(`F8：不移動焦點的按壓位置：${spot.how}（x=${Math.round(spot.x)}, y=${Math.round(spot.y)}）`);
  // 在 document 上計數 pointerdown，確認這次按壓真的觸發了 pointerdown（否則測不到東西）。
  await cdp.eval("window.__pd = 0; document.addEventListener('pointerdown', () => { window.__pd += 1; }, true); true");
  const base = { x: spot.x, y: spot.y, button: 'left', clickCount: 1 };
  await cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: spot.x, y: spot.y });
  await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
  await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
  const pd = await cdp.eval('window.__pd');
  check(pd >= 1, `F8：這次按壓觸發了 document 的 pointerdown（實際 ${pd} 次）`);
  const f0 = await cdp.eval(readFocus);
  check(f0.body === false && f0.identity === before.identity, `F8：按壓之後焦點仍在原按鈕（按壓位置沒有移動焦點；實際 ${JSON.stringify(f0)}）`);

  const v0 = await cdp.eval(versionOf);
  await sleep(800);
  const v1 = await cdp.eval(versionOf);
  check(v1 >= v0 + 3, `F8：其後收到至少 3 份新投影、重畫數次（version ${v0} -> ${v1}）`);
  const after = await cdp.eval(readFocus);
  check(after.body === false && after.identity === before.identity, `F8：背景重畫後焦點仍在同一個 task 的「推進」按鈕上（期望 ${before.identity}；實際 ${JSON.stringify(after)}）`);
  check(after.focusVisible === true, `F8：背景重畫後鍵盤焦點外框仍在（:focus-visible；實際 ${after.focusVisible}）`);
  await cdp.eval("document.getElementById('__nofocus') && document.getElementById('__nofocus').remove(); true");
}

// ---------------------------------------------------------------------------
// D1–D4 通道斷線轉暗（task 4.3；spec「畫面整頁重畫」、design D2）
// ---------------------------------------------------------------------------

// 頁面內：token 的計算顏色（以探針節點取得，不手 key hex→rgb）。
const tokenColors = `(() => {
  const read = (name) => {
    const p = document.createElement('span');
    p.style.color = 'var(--' + name + ')';
    document.body.appendChild(p);
    const c = getComputedStyle(p).color;
    p.remove();
    return c;
  };
  return { dim: read('text-dim'), accent: read('accent'), ok: read('ok'), warn: read('warn'), bad: read('bad'), text: read('text') };
})()`;

// 逐組（selector＋要比的屬性）蒐集計算顏色；頁面內回傳每組的元素數與「不等於 --text-dim」的樣本。
// props：'bg'（背景色，非透明才比）、'border'（四邊，邊框寬度 > 0 才比）、'borderLeft'（左框）、
// 'shadow'（box-shadow 不得含品牌強調色），其餘為一般 CSS 屬性名。
const darkGroups = [
  { name: 'runtime 卡連線狀態（符號＋文字）', sel: '.runtime-card .runtime-conn', props: ['color'], min: 2 },
  { name: 'runtime 卡連線狀態文字節點', sel: '.runtime-card .connection-state', props: ['color'], min: 2 },
  { name: 'runtime 卡連線符號', sel: '.runtime-card .runtime-conn .conn-symbol', props: ['color', 'bg', 'border'], min: 2 },
  { name: 'runtime 卡內 agent 狀態文字（pane／workspace／tab）', sel: '.runtime-card .status', props: ['color'], min: 3 },
  { name: 'runtime 卡內 agent 狀態符號', sel: '.runtime-card .agent-dot', props: ['bg', 'border'], min: 3 },
  { name: 'task 節點狀態色條（左框）', sel: '.task-node', props: ['borderLeft'], min: 3 },
  { name: 'task 節點狀態文字與符號', sel: '.task-node .task-state, .task-node .task-status-label, .task-node .task-status-symbol', props: ['color'], min: 3 },
  { name: 'running／failed／blocked 節點四邊外框', sel: '.task-node.task-status-running, .task-node.task-status-failed, .task-node.task-status-blocked', props: ['border'], min: 1 },
  { name: 'running 節點柔光（box-shadow 不含強調色）', sel: '.task-node.task-status-running', props: ['shadow'], min: 1 },
  { name: '左欄 project 狀態計數', sel: '.project-count, .project-count .project-count-symbol', props: ['color'], min: 2 },
];
const darkSnapshot = (groups) => `(() => {
  const read = (name) => {
    const p = document.createElement('span');
    p.style.color = 'var(--' + name + ')';
    document.body.appendChild(p);
    const c = getComputedStyle(p).color;
    p.remove();
    return c;
  };
  const dim = read('text-dim');
  const accent = read('accent');
  const accentRgb = accent.replace(/^rgb\\(|\\)$/g, '');
  // running 節點柔光是 color-mix 推導的半透明強調色，計算值不是 rgb(...) 字串：用同一條宣告在探針上取計算值比對。
  const glowProbe = document.createElement('span');
  glowProbe.style.boxShadow = '0 0 10px 1px color-mix(in srgb, var(--accent) 40%, transparent)';
  document.body.appendChild(glowProbe);
  const accentGlow = getComputedStyle(glowProbe).boxShadow;
  glowProbe.remove();
  const transparent = (c) => c === 'rgba(0, 0, 0, 0)' || c === 'transparent';
  const sides =['Top', 'Right', 'Bottom', 'Left'];
  const out = [];
  for (const g of ${JSON.stringify(groups)}) {
    const nodes = Array.from(document.querySelectorAll(g.sel.split(',').map((x) => '#app ' + x.trim()).join(', ')));
    const bad = [];
    for (const n of nodes) {
      const cs = getComputedStyle(n);
      const label = (n.className || n.tagName) + ':' + (n.textContent || '').trim().slice(0, 24);
      for (const prop of g.props) {
        const pairs = [];
        if (prop === 'bg') {
          if (!transparent(cs.backgroundColor)) pairs.push(['background-color', cs.backgroundColor]);
        } else if (prop === 'border') {
          for (const sd of sides) {
            if (parseFloat(cs['border' + sd + 'Width']) > 0 && cs['border' + sd + 'Style'] !== 'none') pairs.push(['border-' + sd, cs['border' + sd + 'Color']]);
          }
        } else if (prop === 'borderLeft') {
          pairs.push(['border-left', cs.borderLeftColor]);
        } else if (prop === 'shadow') {
          if (cs.boxShadow !== 'none' && (cs.boxShadow === accentGlow || cs.boxShadow.includes(accentRgb))) bad.push(label + ' box-shadow=' + cs.boxShadow);
          continue;
        } else {
          pairs.push([prop, cs[prop]]);
        }
        for (const [k, v] of pairs) if (v !== dim) bad.push(label + ' ' + k + '=' + v);
      }
    }
    out.push({ name: g.name, count: nodes.length, min: g.min, bad: bad.slice(0, 4), badCount: bad.length });
  }
  return { dim, accent, groups: out };
})()`;

// 頁面內：「未宣告 task」提示（只在特定 Project 出現）。
const undeclaredSnapshot = `(() => {
  const p = document.createElement('span'); p.style.color = 'var(--text-dim)'; document.body.appendChild(p);
  const dim = getComputedStyle(p).color; p.remove();
  const nodes = Array.from(document.querySelectorAll('#app .ff-undeclared'));
  return { count: nodes.length, bad: nodes.filter((n) => getComputedStyle(n).color !== dim).map((n) => getComputedStyle(n).color) };
})()`;

// 頁面內：runtime 卡的「最後已知」手足節點與連線狀態文字。
const runtimeCardStale = `(() => Array.from(document.querySelectorAll('#app .runtime-card')).map((card) => {
  const conn = card.querySelector('.runtime-conn');
  const state = card.querySelector('.connection-state');
  const stale = card.querySelector('.runtime-conn-stale');
  return {
    id: (card.querySelector('.runtime-id') || {}).textContent,
    stateText: state ? state.textContent : null,
    connText: conn ? conn.innerText.replace(/\\s+/g, ' ').trim() : null,
    staleShown: !!stale && getComputedStyle(stale).display !== 'none' && stale.getBoundingClientRect().width > 0,
    staleText: stale ? stale.textContent : null,
    siblingOfState: !!stale && !!state && stale.parentElement === state.parentElement && !state.contains(stale),
  };
}))()`;

// 頁面內：按鈕的計算外觀（不得因斷線而改變）。
const buttonLook = `(() => Array.from(document.querySelectorAll('#app .action-button')).map((b) => {
  const cs = getComputedStyle(b);
  return [b.dataset.action || '', b.textContent.trim(), cs.color, cs.backgroundColor, cs.borderTopColor, cs.opacity, cs.pointerEvents, cs.filter].join('|');
}))()`;

// 還原後：幾個代表性元素的狀態色。
const stateColors = `(() => {
  const color = (sel) => { const n = document.querySelector('#app ' + sel); return n ? getComputedStyle(n).color : null; };
  const border = (sel) => { const n = document.querySelector('#app ' + sel); return n ? getComputedStyle(n).borderTopColor : null; };
  const shadow = (sel) => { const n = document.querySelector('#app ' + sel); return n ? getComputedStyle(n).boxShadow : null; };
  return {
    connState: color('.runtime-conn-connected .connection-state'),
    working: color('.status-working'),
    taskRunning: color('.task-status-running .task-state'),
    taskRunningBorder: border('.task-node.task-status-running'),
    taskRunningShadow: shadow('.task-node.task-status-running'),
    countRunning: color('.project-count-running'),
    undeclared: color('.ff-undeclared'),
  };
})()`;

const appChannelIs = (state) => `document.getElementById('app').getAttribute('data-channel-state') === '${state}'`;

function checkDark(snap, undecl, where, { expectUndeclared }) {
  for (const g of snap.groups) {
    check(g.count >= g.min, `${where}：「${g.name}」至少 ${g.min} 個元素（實際 ${g.count}）`);
    check(g.badCount === 0, `${where}：「${g.name}」計算顏色一律為 --text-dim ${snap.dim}（不符 ${g.badCount} 個：${JSON.stringify(g.bad)}）`);
  }
  if (expectUndeclared) {
    check(undecl.count >= 1, `${where}：列首出現「未宣告 task」提示（實際 ${undecl.count} 個）`);
    check(undecl.bad.length === 0, `${where}：「未宣告 task」提示計算顏色為 --text-dim（不符：${JSON.stringify(undecl.bad)}）`);
  }
}

function checkStale(cards, where, expectShown) {
  check(cards.length >= 2, `${where}：至少兩張 runtime 卡（實際 ${cards.length}）`);
  for (const c of cards) {
    // 連線狀態節點的 textContent 維持精確等於連線狀態字串（既有腳本以此比對）。
    check(['connected', 'connecting', 'disconnected'].includes(c.stateText), `${where}：${c.id} 卡連線狀態節點 textContent 仍是狀態字串本身（實際 ${JSON.stringify(c.stateText)}）`);
    if (expectShown) {
      check(c.staleShown && c.staleText === '最後已知', `${where}：${c.id} 卡「最後已知」可見（實際 ${JSON.stringify(c)}）`);
      check(c.siblingOfState, `${where}：${c.id} 卡「最後已知」是連線狀態節點的手足、不在其內`);
      check(c.connText.includes('最後已知') && c.connText.includes(c.stateText), `${where}：${c.id} 卡連線狀態文字含「最後已知」且仍可讀到 ${c.stateText}（實際 ${JSON.stringify(c.connText)}）`);
    } else {
      check(!c.staleShown, `${where}：${c.id} 卡「最後已知」不可見（實際 ${JSON.stringify(c)}）`);
      check(c.connText !== null && !c.connText.includes('最後已知'), `${where}：${c.id} 卡連線狀態可見文字不含「最後已知」（實際 ${JSON.stringify(c.connText)}）`);
    }
  }
}

async function darkEverywhere(cdp, where, expectUndeclared) {
  checkDark(await cdp.eval(darkSnapshot(darkGroups)), await cdp.eval(undeclaredSnapshot), where, { expectUndeclared });
  checkStale(await cdp.eval(runtimeCardStale), where, true);
}

async function disconnectedCase(cdp, url) {
  log('--- D1 通道斷線轉暗（onChannel 模擬，同 visual-check.js CH1 手法）---');
  await freshLoad(cdp, url);
  // 先把滑鼠移到無互動處，避免 hover 影響按鈕外觀快照。
  await cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 });
  const tok = await cdp.eval(tokenColors);
  check(await cdp.eval(appChannelIs('connected')), 'D1：連線中 #app 的 data-channel-state 為 connected');

  // 連線中的基準：狀態色確實不是 --text-dim（否則下面的斷線斷言是恆真）。
  const live = await cdp.eval(stateColors);
  check(live.connState === tok.ok, `D1：連線中 runtime 卡連線狀態為成功色（實際 ${live.connState}）`);
  check(live.working === tok.accent, `D1：連線中 working 狀態文字為品牌強調色（實際 ${live.working}）`);
  check(live.taskRunning === tok.accent && live.taskRunningBorder === tok.accent, `D1：連線中 running 節點狀態文字與外框為品牌強調色（實際 ${live.taskRunning}／${live.taskRunningBorder}）`);
  check(live.countRunning === tok.accent, `D1：連線中 running 計數為品牌強調色（實際 ${live.countRunning}）`);
  checkStale(await cdp.eval(runtimeCardStale), 'D1 連線中', false);
  const buttonsLive = await cdp.eval(buttonLook);
  check(buttonsLive.length >= 3, `D1：畫面上有按鈕可比對（實際 ${buttonsLive.length} 個）`);

  await cdp.eval("window.onChannel('disconnected'); true");
  check(await cdp.eval(appChannelIs('disconnected')), 'D1：onChannel 後 #app 的 data-channel-state 為 disconnected（不重畫就生效）');
  await darkEverywhere(cdp, 'D1 斷線', false);
  const buttonsDark = await cdp.eval(buttonLook);
  check(
    JSON.stringify(buttonsDark) === JSON.stringify(buttonsLive),
    `D1：斷線時按鈕外觀（顏色、底色、框、opacity、pointer-events、filter）與連線時完全相同（連線 ${JSON.stringify(buttonsLive.slice(0, 2))}；斷線 ${JSON.stringify(buttonsDark.slice(0, 2))}）`
  );

  // 含「未宣告 task」提示的 Project p：左欄點進去（真滑鼠）。
  await cdp.click('[data-action="select-project"][data-project="p"]');
  await cdp.waitFor("!!document.querySelector('#app .ff-undeclared')", 3000, 'D1：切到 Project p 後出現「未宣告 task」提示');
  await darkEverywhere(cdp, 'D1 斷線（Project p）', true);

  log('--- D2 斷線中整頁重畫後仍為最後已知 ---');
  await cdp.eval("window.__paneRow = document.querySelector('.pane-row[data-action=\"select-pane\"]'); true");
  await cdp.click('.pane-row[data-action="select-pane"]');
  await sleep(500);
  check(await cdp.eval("window.__paneRow !== document.querySelector('.pane-row[data-action=\"select-pane\"]')"), 'D2：點 pane 列後整頁已重畫（pane 列換成新節點）');
  check(await cdp.eval(appChannelIs('disconnected')), 'D2：重畫後 #app 的 data-channel-state 仍為 disconnected');
  await darkEverywhere(cdp, 'D2 斷線中點 pane 列重畫後', true);
  // 其後的背景重畫（投影每 100 ms 推送）也不洗掉。
  await sleep(1000);
  await darkEverywhere(cdp, 'D2 其後 1 秒背景重畫後', true);

  // 根節點屬性若被任何路徑丟掉，paint() 要從模組變數寫回（design D2）。
  await cdp.eval("document.getElementById('app').removeAttribute('data-channel-state'); window.repaint(); true");
  check(await cdp.eval(appChannelIs('disconnected')), 'D2：根節點屬性被移除後，paint() 依模組變數寫回 disconnected');
  // connecting 同樣屬於「不是 connected」。
  await cdp.eval("window.onChannel('connecting'); true");
  check(await cdp.eval(appChannelIs('connecting')), 'D2：onChannel(connecting) 後 #app 的 data-channel-state 為 connecting');
  await darkEverywhere(cdp, 'D2 通道 connecting', true);

  // 恢復。
  await cdp.eval("window.onChannel('connected'); true");
  await sleep(300);
  checkStale(await cdp.eval(runtimeCardStale), 'D1 恢復後', false);
  const back = await cdp.eval(stateColors);
  check(back.connState === tok.ok, `D1：恢復後 runtime 卡連線狀態回到成功色（實際 ${back.connState}）`);
  check(back.working === tok.accent, `D1：恢復後 working 狀態文字回到品牌強調色（實際 ${back.working}）`);
  check(back.taskRunning === tok.accent && back.taskRunningBorder === tok.accent, `D1：恢復後 running 節點狀態文字與外框回到品牌強調色（實際 ${back.taskRunning}／${back.taskRunningBorder}）`);
  check(back.taskRunningShadow !== 'none' && back.taskRunningShadow === live.taskRunningShadow, `D1：恢復後 running 節點柔光還原（實際 ${back.taskRunningShadow}）`);
  check(back.countRunning === tok.accent, `D1：恢復後 running 計數回到品牌強調色（實際 ${back.countRunning}）`);
  check(back.undeclared === tok.warn, `D1：恢復後「未宣告 task」提示回到警示色（實際 ${back.undeclared}）`);
  check(await cdp.eval(appChannelIs('connected')), 'D1：恢復後 #app 的 data-channel-state 為 connected');
}

// D3：真的停掉 ui_preview（通道斷線）、再重啟。
async function realDisconnectCase(cdp, url, serverRef) {
  log('--- D3 真的停掉 ui_preview 造成通道斷線，再重啟恢復 ---');
  await freshLoad(cdp, url);
  const tok = await cdp.eval(tokenColors);
  killTree(serverRef.server, 'ui_preview（D3 停掉）');
  const gone = await cdp.waitFor("document.getElementById('app').getAttribute('data-channel-state') !== 'connected'", 8000, 'D3：服務停掉後 #app 的 data-channel-state 離開 connected');
  if (gone) await darkEverywhere(cdp, 'D3 服務停掉後', false);
  serverRef.server = serverRef.start();
  check(await serverRef.waitUp(), 'D3：ui_preview 重啟後開始回應');
  await cdp.waitFor(appChannelIs('connected'), 20000, 'D3：重連後 #app 的 data-channel-state 回到 connected');
  await sleep(500);
  checkStale(await cdp.eval(runtimeCardStale), 'D3 重連後', false);
  const back = await cdp.eval(stateColors);
  check(back.connState === tok.ok && back.working === tok.accent, `D3：重連後狀態色還原（${JSON.stringify(back)}）`);
}

// 截圖去識別化（修正波 2 W2-I2）：ui_preview 的 review fixture 把 pane cwd 放在真實 %TEMP% 底下，
// 畫面會出現 `C:\Users\<真實使用者名稱>\AppData\Local\Temp\…`。截圖前在頁面裝 MutationObserver，
// 把文字節點中 `Users\` 之後的路徑段與真實使用者名稱、主機名稱換成 `<user>`／`<host>`；背景重畫
// 一直重建節點，所以要持續替換。只改文字內容，不動版面。名稱在執行時由 os 取得，不寫進 repo。
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

// 截圖前確認頁面可見文字（textContent，含被 CSS 截斷的部分）不含真實使用者名稱或主機名稱。
async function assertMasked(cdp) {
  const names = [os.userInfo().username, os.hostname()].filter(Boolean);
  const text = await cdp.eval('document.body.textContent');
  const hit = names.filter((n) => text.toLowerCase().includes(n.toLowerCase()));
  check(hit.length === 0, `D4：截圖前頁面文字不含真實使用者名稱與主機名稱（命中 ${hit.length} 個）`);
  return hit.length === 0;
}

// D4：斷線狀態三種 viewport 寬的截圖（設計審核素材）。
async function screenshotCase(cdp, url) {
  log('--- D4 斷線狀態截圖（1536／1100／700）---');
  await freshLoad(cdp, url);
  await cdp.eval(maskExpression());
  await cdp.click('[data-action="select-project"][data-project="p"]');
  await cdp.waitFor("!!document.querySelector('#app .ff-undeclared')", 3000, 'D4：Project p 的「未宣告 task」提示已出現');
  await cdp.eval("window.onChannel('disconnected'); true");
  for (const [w, h] of [[1536, 1300], [1100, 1400], [700, 1400]]) {
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
    await sleep(500);
    if (!(await assertMasked(cdp))) continue;
    const shot = await cdp.send('Page.captureScreenshot', { format: 'png' });
    const file = path.join(__dirname, `ui-fixes-disconnected-${w}.png`);
    fs.writeFileSync(file, Buffer.from(shot.result.data, 'base64'));
    log(`wrote ${file}`);
  }
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  await cdp.eval("window.onChannel('connected'); true");
}

// ---------------------------------------------------------------------------
// B1–B4 斷線覆蓋的改綁標示與取消（task 4.4；spec「Factory Floor」「畫面操作」、design D3）
// ---------------------------------------------------------------------------

// 頁面內：某 workstream 列首的綁定摘要與操作按鈕。徽章（綁定摘要內的 span.ff-binding-badge）與列首
// 「改綁」按鈕（data-action="rebind"）是兩回事，分開讀。
const wsHeaderInfo = (project, ws) => `(() => {
  const proj = document.querySelector('#app .project[data-project="${project}"]');
  const h = document.querySelector('#app .ff-row-header[data-workstream="${ws}"]');
  if (!proj || !h) return null;
  const summary = h.querySelector('.ff-binding');
  const text = summary && summary.querySelector('.ff-binding-text');
  return {
    summaryClass: summary ? summary.className : null,
    summaryText: text ? text.textContent : null,
    badges: summary ? Array.from(summary.querySelectorAll('.ff-binding-badge')).map((b) => b.textContent) : null,
    actions: Array.from(h.querySelectorAll('.ff-row-actions [data-action]')).map((b) => b.dataset.action + ':' + b.textContent.trim()),
    rebindButtons: h.querySelectorAll('.ff-row-actions button[data-action="rebind"]').length,
  };
})()`;

function checkWsHeader(info, where, { expectBadge, expectClear }) {
  check(info !== null, `${where}：找得到該 workstream 列首`);
  if (info === null) return;
  check(info.summaryText === 'runtime 未連線', `${where}：綁定摘要顯示「runtime 未連線」（實際 ${JSON.stringify(info.summaryText)}）`);
  check(info.summaryClass.includes('ff-binding-runtime_disconnected'), `${where}：綁定狀態為 runtime_disconnected（實際 ${info.summaryClass}）`);
  if (expectBadge) {
    check(JSON.stringify(info.badges) === JSON.stringify(['改綁']), `${where}：綁定摘要內有且只有一個「改綁」徽章（實際 ${JSON.stringify(info.badges)}）`);
  } else {
    check(JSON.stringify(info.badges) === '[]', `${where}：綁定摘要內沒有「改綁」徽章（實際 ${JSON.stringify(info.badges)}）`);
  }
  check(info.rebindButtons === 1, `${where}：列首有「改綁」按鈕（與徽章不同；實際 ${info.rebindButtons} 顆）`);
  check(info.actions.includes('rebind:改綁'), `${where}：列首操作含 rebind:改綁（實際 ${JSON.stringify(info.actions)}）`);
  check(
    info.actions.includes('override-clear:取消改綁') === expectClear,
    `${where}：列首${expectClear ? '有' : '沒有'}「取消改綁」按鈕（實際 ${JSON.stringify(info.actions)}）`
  );
}

async function overrideDisconnectedCase(cdp, url, requests) {
  log('--- B1–B4 斷線覆蓋的改綁標示與取消 ---');
  await freshLoad(cdp, url);
  await cdp.click('[data-action="select-project"][data-project="cockpit"]');
  await cdp.waitFor("!!document.querySelector('#app .project[data-project=\"cockpit\"]')", 3000, 'B：選定 Project cockpit');
  await cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 });

  // B1：覆蓋造成的斷線（ovr）：徽章、「改綁」按鈕、「取消改綁」都在。
  checkWsHeader(await cdp.eval(wsHeaderInfo('cockpit', 'ovr')), 'B1 覆蓋斷線 ovr', { expectBadge: true, expectClear: true });
  // B2：自動綁定的斷線（docs）：沒有徽章、沒有「取消改綁」，但有「改綁」按鈕。
  checkWsHeader(await cdp.eval(wsHeaderInfo('cockpit', 'docs')), 'B2 自動斷線 docs', { expectBadge: false, expectClear: false });

  // B3：停在該狀態整頁重畫（window.repaint、點 pane 列、其後的背景重畫）後仍正確。
  await cdp.eval("window.__hdr = document.querySelector('#app .ff-row-header[data-workstream=\"ovr\"]'); true");
  await cdp.eval('window.repaint(); true');
  check(await cdp.eval("window.__hdr !== document.querySelector('#app .ff-row-header[data-workstream=\"ovr\"]')"), 'B3：window.repaint() 後列首已換成新節點（整頁重畫）');
  checkWsHeader(await cdp.eval(wsHeaderInfo('cockpit', 'ovr')), 'B3 repaint 後 ovr', { expectBadge: true, expectClear: true });
  checkWsHeader(await cdp.eval(wsHeaderInfo('cockpit', 'docs')), 'B3 repaint 後 docs', { expectBadge: false, expectClear: false });
  await cdp.click('.pane-row[data-action="select-pane"]');
  await sleep(1000);
  checkWsHeader(await cdp.eval(wsHeaderInfo('cockpit', 'ovr')), 'B3 點 pane 列並歷經背景重畫後 ovr', { expectBadge: true, expectClear: true });
  checkWsHeader(await cdp.eval(wsHeaderInfo('cockpit', 'docs')), 'B3 點 pane 列並歷經背景重畫後 docs', { expectBadge: false, expectClear: false });

  // 另一個 Project 的自動斷線（tests）同樣沒有徽章與「取消改綁」。
  await cdp.click('[data-action="select-project"][data-project="p"]');
  await cdp.waitFor("!!document.querySelector('#app .project[data-project=\"p\"]')", 3000, 'B2：切到 Project p');
  checkWsHeader(await cdp.eval(wsHeaderInfo('p', 'tests')), 'B2 自動斷線 p/tests', { expectBadge: false, expectClear: false });
  await cdp.click('[data-action="select-project"][data-project="cockpit"]');
  await cdp.waitFor("!!document.querySelector('#app .project[data-project=\"cockpit\"]')", 3000, 'B：切回 Project cockpit');

  // B4：按「取消改綁」（真滑鼠）：服務收到恰好一個 DELETE。
  requests.length = 0;
  await cdp.click('[data-action="override-clear"][data-project="cockpit"][data-workstream="ovr"]');
  await sleep(500);
  check(
    requests.length === 1 && requests[0].method === 'DELETE' && requests[0].path === '/api/projects/cockpit/workstreams/ovr/override',
    `B4：按 ovr 的「取消改綁」後服務恰好收到 DELETE /api/projects/cockpit/workstreams/ovr/override（實際 ${JSON.stringify(requests)}）`
  );
  requests.length = 0;
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

async function partPreview() {
  log('=== 滑鼠／鍵盤焦點外框（ui_preview，COCKPIT_PREVIEW_PUSH_MS=100）===');
  if (!fs.existsSync(UI_PREVIEW_EXE)) {
    throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  }
  const port = pickPort(7770);
  if (port !== 7770) log(`7770 已被占用，改用埠 ${port}`);
  const cdpPort = pickPort(18800, [port]);

  const requests = [];
  let chrome = null;
  const serverRef = { server: null };
  serverRef.start = () => {
    const server = spawn(UI_PREVIEW_EXE, [], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true,
      env: {
        ...process.env,
        COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`,
        COCKPIT_PREVIEW_PUSH_MS: '100',
      },
    });
    server.on('error', (e) => check(false, `ui_preview spawn error：${e.message}`));
    let buffer = '';
    server.stdout.setEncoding('utf8');
    server.stdout.on('data', (chunk) => {
      buffer += chunk;
      let nl;
      while ((nl = buffer.indexOf('\n')) !== -1) {
        const line = buffer.slice(0, nl).replace(/\r$/, '');
        buffer = buffer.slice(nl + 1);
        const m = /^write-request (\S+) (\S+) ?(.*)$/.exec(line);
        if (m) requests.push({ method: m[1], path: m[2], body: m[3] });
      }
    });
    return server;
  };
  serverRef.waitUp = async () => {
    for (let i = 0; i < 50; i++) {
      try {
        if ((await fetch(`http://127.0.0.1:${port}/api/state`)).ok) return true;
      } catch {
        // 還沒起來。
      }
      await sleep(200);
    }
    return false;
  };
  try {
    serverRef.server = serverRef.start();
    const up = await serverRef.waitUp();
    check(up, `ui_preview 應該在 10 秒內開始回應（port ${port}）`);
    if (!up) throw new Error('ui_preview 沒有起來');

    const url = `http://127.0.0.1:${port}/`;
    chrome = await startChrome(cdpPort, url, 'ui-fixes');
    const { cdp } = chrome;
    await cdp.waitFor("document.querySelectorAll('.task-node').length >= 9", 5000, '畫出預設 Project 的 task 節點');

    await mouseCase(cdp, url, 'F1', ' pane 列', '.pane-row[data-action="select-pane"]');
    await mouseCase(cdp, url, 'F2', '左欄專案', '[data-action="select-project"]');
    await mouseCase(cdp, url, 'F3', ' task 按鈕（推進）', '[data-action="advance"]');
    await deselectCase(cdp, url);
    await keyboardCase(cdp, url, requests);
    await mouseThenTabCase(cdp, url);
    await modifierKeyCase(cdp, url);
    await pointerWithoutFocusMoveCase(cdp, url);
    await overrideDisconnectedCase(cdp, url, requests);
    await disconnectedCase(cdp, url);
    if (process.argv.includes('--screenshots')) await screenshotCase(cdp, url);
    await realDisconnectCase(cdp, url, serverRef);
  } finally {
    await stopChrome(chrome, 'chrome-ui-fixes');
    killTree(serverRef.server, 'ui_preview');
    await sleep(300);
    check(!isPortListening(port), `port ${port} 應該不再有 LISTENING 的行程`);
  }
}

async function main() {
  if (!fs.existsSync(CHROME)) {
    throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  }
  try {
    await partPreview();
  } catch (e) {
    check(false, `中止：${e.message}`);
  }
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
