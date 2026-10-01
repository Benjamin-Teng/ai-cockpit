// live-output-color 驗收（Live Output 面板依樣式上色）：新斷言集中在這支腳本，用法與各段對應 scenario
// 見同目錄 output-color-check.md。寫法比照 docs/research/2026-10-01/ui-fixes-check.js（raw CDP、
// headless Chrome、依 PID 收尾）。
//
// 涵蓋（live-output-color task 5.3；spec live-output、design D7／D8）：
//   A0–A10 「輸出依樣式上色」全部 scenario：全組合對照表（含 16 色前景／背景各一格）、紅字與綠底、有色字疊在淡底上、反白優先於背景、
//          黑與亮黑背景、粗體／斜體／底線、變暗、上色後內容仍不被當成 HTML、不認得的顏色名稱、對比（Chrome 計算後
//          顏色合成到 --bg-deep 量 WCAG 對比 >= 4.5:1）。
//   T1     「輪詢與顯示」：面板 textContent 等於回應 text。
//   P1     「相同內容不重寫」（MutationObserver 確認無變動）。
//   M1     「輪詢與顯示」：200 回應本體不是 JSON 物件（null 等）時輪詢不中止、內容不被清空（5.6 F2）。
//   X1     「輪詢與顯示」：segments 與 text 不一致時，面板 textContent 仍等於回應 text（5.6 F3；design D7）。
//   P2     「只有顏色改變也會重畫」（ansi-flip）。
//   S1     「失敗與消失的呈現」的「過期時有色內容一併轉暗」。
//
// 預期值一律取自頁面上的色票（getComputedStyle 的 --bad 等），不手 key hex；只比計算後顏色與字重／字型樣式，
// 不比 class 名稱（class 是實作細節）。
//
// 用法（repo 根，需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-10-02/output-color-check.js [--screenshots]
// --screenshots（live-output-color task 5.4）：全部斷言跑完後，選 ansi pane，產生 1536／1100／700 寬的
// 截圖 output-color-<寬>.png（去識別化作法同 ui-fixes-check.js）。
// 清理：只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID），埠被占用就往上找空埠。
const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

// ansi 樣本掛在 wJ:p1，ansi-flip 掛在 wJ:p3（兩者都是投影裡可點選的 pane）。
const ANSI_PANE = 'wJ:p1';
const FLIP_PANE = 'wJ:p3';
const RUNTIME = 'win';

const failures = [];
function check(cond, label) {
  console.log(`${cond ? 'PASS' : 'FAIL'} ${label}`);
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

// 任何本機位址（127.0.0.1、0.0.0.0、[::] 等）上該埠有 LISTENING 都算：只看 127.0.0.1 會漏掉別人以
// wildcard 位址監聽的情況（把它誤判成空埠，收尾檢查也會漏看）。netstat -ano 欄位：協定、本機位址、
// 遠端位址、狀態、PID；本機位址以 `:<port>` 結尾。
function isPortListening(port) {
  const r = spawnSync('netstat', ['-ano'], { encoding: 'utf8' });
  return (r.stdout || '').split('\n').some((line) => {
    const f = line.trim().split(/\s+/);
    return f.length >= 4 && f[1].endsWith(`:${port}`) && f.includes('LISTENING');
  });
}

function pickPort(start, avoid = []) {
  let port = start;
  while (isPortListening(port) || avoid.includes(port)) port += 1;
  return port;
}

// 還有沒有 chrome.exe 的命令列含這個 user-data-dir（本腳本只依 PID 殺，這裡是事後確認沒有殘留）。
function chromeWithUserDataDirAlive(udd) {
  const r = spawnSync(
    'powershell',
    [
      '-NoProfile',
      '-Command',
      `@(Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.CommandLine -like '*${udd.replace(/'/g, "''")}*' }).Count`,
    ],
    { encoding: 'utf8' }
  );
  return Number((r.stdout || '').trim()) > 0;
}

class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.handlers = new Map();
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        this.pending.get(m.id)(m);
        this.pending.delete(m.id);
      } else if (m.method && this.handlers.has(m.method)) {
        this.handlers.get(m.method)(m.params);
      }
    };
  }
  on(method, handler) {
    this.handlers.set(method, handler);
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
  // 以真的滑鼠事件點 selector 指到的元素中心。scrollIntoView 之後等 100 ms 再送滑鼠事件
  // （docs/research/2026-09-19/live-output-check.js 查到的合成器競態）。
  async click(selector) {
    const exists = await this.eval(
      `(() => { const n = document.querySelector(${JSON.stringify(selector)});
        if (!n) return false; n.scrollIntoView({block: 'center'}); return true; })()`
    );
    if (!exists) {
      check(false, `找不到可點的元素：${selector}`);
      return false;
    }
    await sleep(100);
    const rect = await this.eval(
      `(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();
        return {x: r.left + r.width / 2, y: r.top + r.height / 2}; })()`
    );
    const base = { x: rect.x, y: rect.y, button: 'left', clickCount: 1 };
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x, y: rect.y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base });
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base });
    return true;
  }
}

async function startChrome(cdpPort, url, label) {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), `cockpit-chrome-${label}-`));
  const chrome = spawn(
    CHROME,
    [
      '--headless=new',
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
  // spawn 之後的等待（CDP target、WebSocket）若拋出，呼叫端還拿不到 handle、finally 的 stopChrome 收不到
  // 這個 chrome，所以失敗時要在這裡先收掉自己開的 chrome 與暫存 user-data-dir 再拋出。
  try {
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
    return { chrome, udd, ws, cdp: new CDP(ws), cdpPort };
  } catch (e) {
    killTree(chrome, label);
    await sleep(500);
    try {
      fs.rmSync(udd, { recursive: true, force: true });
    } catch {
      // 暫存目錄仍被占用就留給 OS 清；上面的 killTree 已確認程序終止。
    }
    throw e;
  }
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
  check(!chromeWithUserDataDirAlive(handle.udd), `${label}：沒有殘留使用本腳本 user-data-dir 的 chrome.exe`);
  check(!isPortListening(handle.cdpPort), `CDP port ${handle.cdpPort} 應該不再有 LISTENING 的行程`);
  try {
    fs.rmSync(handle.udd, { recursive: true, force: true });
  } catch (e) {
    check(false, `清理暫存目錄失敗：${e.message}`);
  }
}

// ---------------------------------------------------------------------------
// 樣本與對照表（與 cockpit/examples/ui_preview.rs `ansi_sample` 的文件註解一一對應）
// ---------------------------------------------------------------------------

// spec「輸出依樣式上色」的前景對照；背景用同一張表（淡底取同一色票）。
// 16 色全部列出：bright_* 與基本色同一個色票；前景 black 與 bright_black 都是 --text-dim
// （背景的 black 另外不畫，見 expectedFor）。
const TOKEN = {
  black: 'text-dim',
  red: 'bad',
  green: 'ok',
  yellow: 'warn',
  blue: 'accent',
  magenta: 'graph-lane-4',
  cyan: 'accent',
  white: 'text',
  bright_black: 'text-dim',
  bright_red: 'bad',
  bright_green: 'ok',
  bright_yellow: 'warn',
  bright_blue: 'accent',
  bright_magenta: 'graph-lane-4',
  bright_cyan: 'accent',
  bright_white: 'text',
};
// SGR 碼順序：30–37、90–97（背景 +10）。
const COLORS16 = Object.keys(TOKEN);
const PALETTE = ['red', 'green', 'yellow', 'blue', 'magenta', 'white', 'bright_black'];
const NAMES = ['none', ...PALETTE];

// 每個有樣式片段的樣式描述（fg/bg 為 null 表示未指定）。
const SAMPLES = [];
for (const bg of NAMES) {
  for (const fg of NAMES) {
    SAMPLES.push({
      label: `[fg=${fg} bg=${bg}]`,
      group: 'grid',
      fg: fg === 'none' ? null : fg,
      bg: bg === 'none' ? null : bg,
    });
  }
}
for (const fg of NAMES) {
  SAMPLES.push({ label: `[rev fg=${fg}]`, group: 'rev', fg: fg === 'none' ? null : fg, rev: true });
}
// 16 種前景（30–37、90–97）各一格、16 種背景（40–47、100–107）各一格（design D8；5.6 F1）。
for (const c of COLORS16) SAMPLES.push({ label: `[fg16=${c}]`, group: 'fg16', fg: c });
for (const c of COLORS16) SAMPLES.push({ label: `[bg16=${c}]`, group: 'bg16', bg: c });
SAMPLES.push(
  { label: '[bg=black]', group: 'misc', bg: 'black' },
  { label: '[bg=bright_black]', group: 'misc', bg: 'bright_black' },
  { label: '[fg=red bg=white rev]', group: 'misc', fg: 'red', bg: 'white', rev: true },
  { label: '[bold]', group: 'misc', bold: true },
  { label: '[italic]', group: 'misc', italic: true },
  { label: '[underline]', group: 'misc', underline: true },
  { label: '[dim]', group: 'misc', dim: true },
  { label: '<script>window.pwned=1</script>', group: 'misc', fg: 'red' },
  { label: '<b>x</b>', group: 'misc', bold: true },
  { label: '[fg=cyan]', group: 'misc', fg: 'cyan' },
  { label: '[bg=cyan]', group: 'misc', bg: 'cyan' },
  { label: '[fg=256-196]', group: 'misc', fg: 'red' },
  { label: '[fg=rgb-215-119-87]', group: 'misc', fg: 'yellow' }
);

// 依 spec 算預期：文字色票、背景（null＝不畫，或 {tok, alpha}）。
function expectedFor(s) {
  const textTok = s.fg ? TOKEN[s.fg] : s.dim ? 'text-dim' : 'text';
  let bg = null;
  if (s.rev) bg = { tok: textTok, alpha: 0.2 };
  else if (s.bg && s.bg !== 'black') bg = { tok: TOKEN[s.bg], alpha: s.fg ? 0.14 : 0.2 };
  const styled = !!(s.fg || s.bg || s.rev || s.bold || s.italic || s.underline || s.dim);
  return { textTok, bg, bold: !!s.bold, italic: !!s.italic, underline: !!s.underline, styled };
}

const TOKEN_NAMES = ['bad', 'ok', 'warn', 'accent', 'graph-lane-4', 'text', 'text-dim', 'bg-deep'];

// ---------------------------------------------------------------------------
// 頁面內函式（以 toString() 注入頁面執行，不能引用外層變數）
// ---------------------------------------------------------------------------

// 回傳：tokens（各色票的 {r,g,b,a}）、panelText、items（每個標籤找到的元素的計算樣式）。
function pageProbe(labels, tokenNames) {
  const canvas = document.createElement('canvas');
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  const alphaOf = (v) => {
    if (v === undefined || v === '') return 1;
    return v.endsWith('%') ? parseFloat(v) / 100 : parseFloat(v);
  };
  // rgb()/rgba()/color(srgb ...) 直接解析（保留原始 alpha）；其他寫法（oklab 等）交給 canvas 換算。
  const parseColor = (s) => {
    let m = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/.exec(s);
    if (m) return { r: +m[1], g: +m[2], b: +m[3], a: alphaOf(m[4]) };
    m = /^color\(srgb\s+([\d.e-]+)\s+([\d.e-]+)\s+([\d.e-]+)(?:\s*\/\s*([\d.]+%?))?\s*\)$/.exec(s);
    if (m) return { r: +m[1] * 255, g: +m[2] * 255, b: +m[3] * 255, a: alphaOf(m[4]) };
    if (s === 'transparent') return { r: 0, g: 0, b: 0, a: 0 };
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = '#000';
    ctx.fillStyle = s;
    ctx.fillRect(0, 0, 1, 1);
    const d = ctx.getImageData(0, 0, 1, 1).data;
    return { r: d[0], g: d[1], b: d[2], a: d[3] / 255, via: 'canvas', raw: s };
  };
  const tokenColor = (name) => {
    const p = document.createElement('span');
    p.style.color = `var(--${name})`;
    document.body.appendChild(p);
    const c = getComputedStyle(p).color;
    p.remove();
    return parseColor(c);
  };
  const tokens = {};
  for (const n of tokenNames) tokens[n] = tokenColor(n);

  const pre = document.querySelector('#output .output-text');
  if (!pre) return { tokens, error: '找不到 #output .output-text' };
  const find = (label) => {
    const w = document.createTreeWalker(pre, NodeFilter.SHOW_TEXT);
    let partial = null;
    for (let n = w.nextNode(); n; n = w.nextNode()) {
      if (n.nodeValue === label) return { el: n.parentElement, exact: true };
      if (!partial && n.nodeValue.indexOf(label) >= 0) partial = n.parentElement;
    }
    return partial ? { el: partial, exact: false } : null;
  };
  // 內容框的實際底色：從 .output-text 沿祖先鏈往上收集有顏色的背景層，直到第一個不透明的為止，
  // 再由下往上合成（都透明就以瀏覽器預設白底為底）；不寫死任何色票。
  const layers = [];
  for (let n = pre; n; n = n.parentElement) {
    const c = parseColor(getComputedStyle(n).backgroundColor);
    if (c.a > 0) layers.push(c);
    if (c.a >= 0.999) break;
  }
  let panelBase = { r: 255, g: 255, b: 255, a: 1 };
  for (let i = layers.length - 1; i >= 0; i -= 1) {
    const c = layers[i];
    panelBase = {
      r: c.r * c.a + panelBase.r * (1 - c.a),
      g: c.g * c.a + panelBase.g * (1 - c.a),
      b: c.b * c.a + panelBase.b * (1 - c.a),
      a: 1,
    };
  }
  const items = {};
  for (const label of labels) {
    const f = find(label);
    if (!f) {
      items[label] = { found: false };
      continue;
    }
    const cs = getComputedStyle(f.el);
    items[label] = {
      found: true,
      exact: f.exact,
      isPre: f.el === pre,
      tag: f.el.tagName,
      color: parseColor(cs.color),
      bg: parseColor(cs.backgroundColor),
      weight: cs.fontWeight,
      fontStyle: cs.fontStyle,
      deco: cs.textDecorationLine,
    };
  }
  return { tokens, panelText: pre.textContent, panelBase, items };
}

// 面板（#output）內是否有元素的 class／style 屬性含 needles 之一；回傳命中的屬性清單。
function pageAttrLeaks(needles) {
  const hits = [];
  for (const el of document.querySelectorAll('#output, #output *')) {
    for (const name of ['class', 'style']) {
      const v = el.getAttribute(name);
      if (v && needles.some((n) => v.toLowerCase().includes(n))) hits.push(`${el.tagName}[${name}=${v}]`);
    }
  }
  return hits;
}

// 面板內的 script／b 元素數量，與 window.pwned。
function pageHtmlProbe() {
  return {
    pwned: typeof window.pwned,
    scripts: document.querySelectorAll('#output script').length,
    bolds: document.querySelectorAll('#output b').length,
    // design D7：不使用 style 屬性（樣式只能來自固定 class）。
    styleAttrs: document.querySelectorAll('#output .output-text [style]').length,
  };
}

const probeExpr = (labels) => `(${pageProbe.toString()})(${JSON.stringify(labels)}, ${JSON.stringify(TOKEN_NAMES)})`;

// ---------------------------------------------------------------------------
// 顏色比對與對比計算
// ---------------------------------------------------------------------------

const RGB_TOL = 2; // 每個通道 ±2/255
const ALPHA_TOL = 0.01;

const near = (c, tok, alpha) =>
  Math.abs(c.r - tok.r) <= RGB_TOL &&
  Math.abs(c.g - tok.g) <= RGB_TOL &&
  Math.abs(c.b - tok.b) <= RGB_TOL &&
  Math.abs(c.a - alpha) <= ALPHA_TOL;
const fmt = (c) => (c ? `rgba(${c.r.toFixed(1)},${c.g.toFixed(1)},${c.b.toFixed(1)},${c.a.toFixed(3)})` : 'null');

// 色 c 疊在不透明底 base 上的結果。
function over(c, base) {
  return {
    r: c.r * c.a + base.r * (1 - c.a),
    g: c.g * c.a + base.g * (1 - c.a),
    b: c.b * c.a + base.b * (1 - c.a),
    a: 1,
  };
}
function luminance(c) {
  const lin = (v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
}
function contrast(fg, bg) {
  const a = luminance(fg);
  const b = luminance(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

// 一個標籤的計算樣式對照預期；回傳不符說明（空陣列＝符合）。
function problemsOf(item, exp, tokens) {
  if (!item || !item.found) return ['找不到該片段文字'];
  const out = [];
  // 有樣式的片段必須是獨立的文字節點（標籤文字完全相等）；包含但不相等代表片段被合併或切開。
  // 無樣式的 `[fg=none bg=none]` 與其後的換行同屬一個未上色片段，不在此限。
  if (exp.styled && !item.exact) out.push('找不到文字完全等於標籤的節點（片段被合併或切開，只能部分比對）');
  if (!near(item.color, tokens[exp.textTok], 1)) {
    out.push(`文字色 ${fmt(item.color)}，應為 --${exp.textTok} ${fmt(tokens[exp.textTok])}`);
  }
  if (exp.bg) {
    if (!near(item.bg, tokens[exp.bg.tok], exp.bg.alpha)) {
      out.push(`背景 ${fmt(item.bg)}，應為 --${exp.bg.tok} 的 ${exp.bg.alpha * 100}% 淡底`);
    }
  } else if (item.bg.a > ALPHA_TOL) {
    out.push(`背景 ${fmt(item.bg)}，應為不畫背景`);
  }
  if (exp.bold !== (item.weight === '600')) out.push(`字重 ${item.weight}，bold 應為 ${exp.bold}`);
  if (exp.italic !== (item.fontStyle === 'italic')) out.push(`font-style ${item.fontStyle}，italic 應為 ${exp.italic}`);
  if (exp.underline !== item.deco.includes('underline')) out.push(`text-decoration-line ${item.deco}，underline 應為 ${exp.underline}`);
  return out;
}

async function probe(cdp, labels) {
  return cdp.eval(probeExpr(labels));
}

// ---------------------------------------------------------------------------
// A：輸出依樣式上色（ansi 模式的固定樣本）
// ---------------------------------------------------------------------------

async function selectPane(cdp, pane) {
  await cdp.click(`.pane-row[data-runtime="${RUNTIME}"][data-pane="${pane}"]`);
}

// 面板已畫出 ansi 樣本（最後一個標籤出現）。
const ansiRendered = "(() => { const t = document.querySelector('#output .output-text'); return !!t && t.textContent.includes('[fg=rgb-215-119-87]'); })()";

async function colorCase(cdp) {
  log('--- A 輸出依樣式上色（ansi 樣本）---');
  const labels = SAMPLES.map((s) => s.label);
  const byLabel = Object.fromEntries(SAMPLES.map((s) => [s.label, s]));
  const snap = await probe(cdp, labels);
  check(!snap.error, `A：面板內容框存在（${snap.error || 'ok'}）`);
  if (snap.error) return;
  const { tokens, items } = snap;
  const prob = (label) => problemsOf(items[label], expectedFor(byLabel[label]), tokens);

  // A0 全組合與其餘片段逐一對照（聚合成每組一條，不符時列出前幾個）。
  for (const [group, title] of [
    ['grid', '全組合 64 格（前景 none＋7 種 × 背景 none＋7 種）'],
    ['rev', '反白 8 個（每種前景色票與無前景）'],
    ['fg16', '16 種前景（30–37、90–97，含 black 與全部 bright_*）'],
    ['bg16', '16 種背景（40–47、100–107，含 black 與全部 bright_*）'],
    ['misc', '其餘片段（黑／亮黑背景、cyan 背景、反白疊背景、粗斜底線變暗、HTML 字樣、256 色與真彩色）'],
  ]) {
    const group_s = SAMPLES.filter((s) => s.group === group);
    const bad = group_s.map((s) => ({ label: s.label, p: prob(s.label) })).filter((x) => x.p.length);
    check(
      bad.length === 0,
      `A0：${title}的計算後文字色／底色／字重／斜體／底線都符合 spec 對照表（${group_s.length} 個，不符 ${bad.length}：${JSON.stringify(bad.slice(0, 3))}）`
    );
  }

  // A1 紅字與綠底。
  const red = items['[fg=red bg=none]'];
  check(red.found && near(red.color, tokens.bad, 1), `A1 紅字：fg red 的片段計算後文字色等於 --bad（實際 ${fmt(red.color)}；--bad ${fmt(tokens.bad)}）`);
  const green = items['[fg=none bg=green]'];
  check(
    green.found && near(green.bg, tokens.ok, 0.2) && near(green.color, tokens.text, 1),
    `A1 綠底：bg green 的片段計算後背景色等於 --ok 的 20% 不透明度、文字色為 --text（實際背景 ${fmt(green.bg)}、文字 ${fmt(green.color)}；--ok ${fmt(tokens.ok)}）`
  );

  // A2 有色字疊在淡底上。
  const fgOnBg = items['[fg=red bg=white]'];
  check(
    fgOnBg.found && near(fgOnBg.bg, tokens.text, 0.14) && near(fgOnBg.color, tokens.bad, 1),
    `A2 有色字疊在淡底上：fg red＋bg white 的背景為 --text 的 14%、文字為 --bad（實際背景 ${fmt(fgOnBg.bg)}、文字 ${fmt(fgOnBg.color)}）`
  );
  const noFgOnBg = items['[fg=none bg=white]'];
  check(
    noFgOnBg.found && near(noFgOnBg.bg, tokens.text, 0.2),
    `A2 對照：只有 bg white、沒有前景的片段背景為 --text 的 20%（實際 ${fmt(noFgOnBg.bg)}）`
  );

  // A3 反白優先於背景。
  const revFull = items['[fg=red bg=white rev]'];
  check(
    revFull.found && near(revFull.bg, tokens.bad, 0.2) && near(revFull.color, tokens.bad, 1),
    `A3 反白優先於背景：fg red＋bg white＋reverse 的背景為 --bad 的 20%、文字為 --bad（實際背景 ${fmt(revFull.bg)}、文字 ${fmt(revFull.color)}）`
  );
  const revOnly = items['[rev fg=none]'];
  check(
    revOnly.found && near(revOnly.bg, tokens.text, 0.2),
    `A3 只帶 reverse 的片段背景為 --text 的 20%（實際 ${fmt(revOnly.bg)}）`
  );

  // A4 黑與亮黑背景。
  const bgBlack = items['[bg=black]'];
  check(bgBlack.found && bgBlack.bg.a <= ALPHA_TOL, `A4 bg black 沒有背景色（實際 ${fmt(bgBlack.bg)}）`);
  const bgBright = items['[bg=bright_black]'];
  check(
    bgBright.found && near(bgBright.bg, tokens['text-dim'], 0.2),
    `A4 bg bright_black 的背景為 --text-dim 的 20%（實際 ${fmt(bgBright.bg)}；--text-dim ${fmt(tokens['text-dim'])}）`
  );

  // A5 粗體、斜體、底線（文字色皆為 --text）。
  const b = items['[bold]'];
  const i = items['[italic]'];
  const u = items['[underline]'];
  check(
    b.found && b.weight === '600' && near(b.color, tokens.text, 1),
    `A5 bold：字重 600、文字 --text（實際 weight ${b.weight}、${fmt(b.color)}）`
  );
  check(
    i.found && i.fontStyle === 'italic' && near(i.color, tokens.text, 1),
    `A5 italic：font-style italic、文字 --text（實際 ${i.fontStyle}、${fmt(i.color)}）`
  );
  check(
    u.found && u.deco.includes('underline') && near(u.color, tokens.text, 1),
    `A5 underline：text-decoration-line 含 underline、文字 --text（實際 ${u.deco}、${fmt(u.color)}）`
  );

  // A6 變暗的預設色文字：--text-dim；有前景色的片段不受 dim 影響（樣本沒有 dim＋fg，由 A0 的 fg 片段維持前景色涵蓋）。
  const dim = items['[dim]'];
  check(dim.found && near(dim.color, tokens['text-dim'], 1), `A6 dim 無前景色：文字為 --text-dim（實際 ${fmt(dim.color)}）`);

  // A7 上色後內容仍不被當成 HTML。
  const html = await cdp.eval(`(${pageHtmlProbe.toString()})()`);
  check(html.pwned === 'undefined', `A7：window.pwned 未被設定（typeof ${html.pwned}）`);
  check(html.scripts === 0 && html.bolds === 0, `A7：面板內沒有 script 或 b 元素（script ${html.scripts}、b ${html.bolds}）`);
  check(html.styleAttrs === 0, `A8（design D7）：全部 ${SAMPLES.length} 個有樣式片段畫出之後，內容框內沒有任何元素帶 style 屬性（實際 ${html.styleAttrs} 個）`);
  const text = snap.panelText;
  check(
    text.includes('<script>window.pwned=1</script>') && text.includes('<b>x</b>'),
    'A7：兩段字樣原樣以文字出現在面板 textContent'
  );
  const scriptSeg = items['<script>window.pwned=1</script>'];
  const boldSeg = items['<b>x</b>'];
  check(scriptSeg.found && near(scriptSeg.color, tokens.bad, 1), `A7：fg red 的 <script> 字樣以 --bad 顯示（實際 ${fmt(scriptSeg.color)}）`);
  check(boldSeg.found && boldSeg.weight === '600', `A7：bold 的 <b>x</b> 字樣字重 600（實際 ${boldSeg.weight}）`);

  // A10 歸色（256 色與真彩色已在後端歸為 red／yellow／cyan，這裡確認前端畫出對應色票）。
  const c256 = items['[fg=256-196]'];
  const ctrue = items['[fg=rgb-215-119-87]'];
  const ccyan = items['[fg=cyan]'];
  check(
    c256.found && near(c256.color, tokens.bad, 1) && ctrue.found && near(ctrue.color, tokens.warn, 1) && ccyan.found && near(ccyan.color, tokens.accent, 1),
    `A10：256 色 196 為 --bad、真彩色 #d77757 為 --warn、cyan 為 --accent（實際 ${fmt(c256.color)}／${fmt(ctrue.color)}／${fmt(ccyan.color)}）`
  );

  // A9 對比：全組合、反白、其餘片段的文字色對其實際背景（淡底合成到 --bg-deep）。
  // 底色取內容框祖先鏈實際的不透明背景（pageProbe 的 panelBase），不假設它是哪個色票。
  const panelBase = snap.panelBase;
  const rows = SAMPLES.filter((s) => items[s.label].found).map((s) => {
    const it = items[s.label];
    const base = it.bg.a > 0 ? over(it.bg, panelBase) : panelBase;
    return { label: s.label, ratio: contrast(over(it.color, base), base), colorKey: fmt(it.color), bgKey: fmt(base) };
  });
  const low = rows.filter((r) => r.ratio < 4.5);
  const worst = rows.reduce((a, r) => (a === null || r.ratio < a.ratio ? r : a), null);
  check(
    rows.length === SAMPLES.length && low.length === 0,
    `A9 對比：${rows.length}／${SAMPLES.length} 個片段文字色對實際背景（面板底色實測 ${fmt(panelBase)}）皆 >= 4.5:1（最低 ${worst ? worst.ratio.toFixed(2) : 'n/a'}:1 於 ${worst ? worst.label : 'n/a'}；不足 ${low.length}：${JSON.stringify(low.slice(0, 3))}）`
  );
  // 防止「全是預設色、對比恆過」的空轉：量測集合必須真的涵蓋多種前景色與多種底色。
  const distinctFg = new Set(rows.map((r) => r.colorKey)).size;
  const distinctBg = new Set(rows.map((r) => r.bgKey)).size;
  check(
    distinctFg >= 7 && distinctBg >= 8,
    `A9 涵蓋：量測集合含 7 種色票文字色（--text、--text-dim、--bad、--ok、--warn、--accent、--graph-lane-4）與至少 8 種不同底色（實際 ${distinctFg} 種文字色、${distinctBg} 種底色）`
  );
}

// ---------------------------------------------------------------------------
// T1 textContent 等於回應 text
// ---------------------------------------------------------------------------

async function textContentCase(cdp) {
  log('--- T1 面板 textContent 等於回應 text ---');
  const expr = `(async () => {
    const r = await fetch('/api/runtimes/${RUNTIME}/panes/${encodeURIComponent(ANSI_PANE)}/output', { cache: 'no-store' });
    const body = await r.json();
    const pre = document.querySelector('#output .output-text');
    return {
      status: r.status,
      text: body.text,
      joined: Array.isArray(body.segments) ? body.segments.map((s) => s.text).join('') : null,
      segCount: Array.isArray(body.segments) ? body.segments.length : -1,
      panel: pre ? pre.textContent : null,
    };
  })()`;
  const r0 = await cdp.send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  const v = r0.result.result.value;
  check(v && v.status === 200 && v.segCount >= 80, `T1：端點回 200 且 segments 有 >= 80 段（實際 status ${v && v.status}、${v && v.segCount} 段）`);
  check(v && v.joined === v.text, 'T1：segments 的 text 依序串接等於回應 text');
  check(v && v.panel === v.text, `T1：面板 textContent 等於回應 text（面板 ${v && v.panel && v.panel.length} 字、回應 ${v && v.text && v.text.length} 字）`);
}

// ---------------------------------------------------------------------------
// P1 相同內容不重寫
// ---------------------------------------------------------------------------

async function noRewriteCase(cdp, requests) {
  log('--- P1 相同內容不重寫（MutationObserver）---');
  await cdp.eval(`(() => {
    const pre = document.querySelector('#output .output-text');
    window.__pre0 = pre;
    window.__kids0 = Array.from(pre.childNodes);
    window.__muts = [];
    window.__mo = new MutationObserver((records) => { for (const r of records) window.__muts.push(r.type); });
    window.__mo.observe(pre, { childList: true, subtree: true, characterData: true, attributes: true });
    return true;
  })()`);
  const before = requests.length;
  const start = Date.now();
  while (requests.length - before < 3 && Date.now() - start < 8000) await sleep(100);
  await sleep(300);
  const polls = requests.length - before;
  check(polls >= 3, `P1：觀察期間服務收到至少 3 次輸出請求（輪詢 1 秒一次；實際 ${polls} 次）`);
  const res = await cdp.eval(`(() => {
    const pre = document.querySelector('#output .output-text');
    const kids = Array.from(pre.childNodes);
    return {
      muts: window.__muts.length,
      samePre: pre === window.__pre0,
      sameKids: kids.length === window.__kids0.length && kids.every((k, i) => k === window.__kids0[i]),
      kids: kids.length,
    };
  })()`);
  await cdp.eval('window.__mo && window.__mo.disconnect(); true');
  check(res.muts === 0, `P1：內容框沒有任何 childList／characterData／attributes 變動（實際 ${res.muts} 筆）`);
  check(res.samePre && res.sameKids, `P1：內容框與其子節點仍是原本的節點（沒有被替換；子節點 ${res.kids} 個）`);
}

// ---------------------------------------------------------------------------
// 攔截輸出請求（Fetch domain）：回指定狀態與本體，或放行
// ---------------------------------------------------------------------------

function makeIntercept(cdp) {
  const state = { mode: null, enabled: false, hits: 0 };
  cdp.on('Fetch.requestPaused', (p) => {
    const m = state.mode;
    if (!m || !/\/api\/runtimes\/[^/]+\/panes\/[^/]+\/output/.test(p.request.url)) {
      cdp.send('Fetch.continueRequest', { requestId: p.requestId });
      return;
    }
    state.hits += 1; // 被我們回應的輸出請求數（前端確實還在輪詢的證據）。
    cdp.send('Fetch.fulfillRequest', {
      requestId: p.requestId,
      responseCode: m.status,
      responseHeaders: [
        { name: 'Content-Type', value: 'application/json' },
        { name: 'Cache-Control', value: 'no-store' },
      ],
      body: Buffer.from(m.body, 'utf8').toString('base64'),
    });
  });
  return {
    async enable() {
      await cdp.send('Fetch.enable', { patterns: [{ urlPattern: '*/api/runtimes/*/output*', requestStage: 'Request' }] });
      state.enabled = true;
    },
    async disable() {
      state.mode = null;
      if (state.enabled) await cdp.send('Fetch.disable');
      state.enabled = false;
    },
    set(mode) {
      state.mode = mode;
    },
    hits() {
      return state.hits;
    },
  };
}

// ---------------------------------------------------------------------------
// S1 過期時有色內容一併轉暗
// ---------------------------------------------------------------------------

async function staleCase(cdp, icpt) {
  log('--- S1 過期時有色內容一併轉暗 ---');
  const allLabels = SAMPLES.map((s) => s.label);
  const pair = ['[fg=red bg=none]', '[fg=none bg=green]'];
  // GIVEN：面板顯示含 fg red 與 bg green 片段的輸出（503 之前就要有色）。
  const live = await probe(cdp, allLabels);
  const t = live.tokens;
  check(
    live.items[pair[0]].found && near(live.items[pair[0]].color, t.bad, 1) && live.items[pair[1]].found && near(live.items[pair[1]].bg, t.ok, 0.2),
    `S1 GIVEN：503 之前 fg red 片段為 --bad 文字、bg green 片段為 --ok 20% 淡底（實際 ${fmt(live.items[pair[0]].color)}／${fmt(live.items[pair[1]].bg)}）`
  );
  await icpt.enable();
  icpt.set({ status: 503, body: '{"error":"x"}' });
  const stale = await cdp.waitFor("document.getElementById('output').classList.contains('is-stale')", 6000, 'S1：輸出請求回 503 後面板標為過期');
  if (stale) await sleep(300);
  const dark = await probe(cdp, allLabels);
  const notDim = allLabels.filter((l) => {
    const it = dark.items[l];
    return !it.found || !near(it.color, t['text-dim'], 1) || it.bg.a > ALPHA_TOL;
  });
  const p0 = dark.items[pair[0]];
  const p1 = dark.items[pair[1]];
  // 「兩個片段」必須是面板裡各自獨立的節點（不是同一個 <pre> 底下的一整串文字），否則「全部轉暗」是舊行為本來就成立
  // 的空話：舊前端整個內容框單一顏色，轉暗斷言本來就綠，要靠這個條件與 GIVEN／恢復斷言才辨識得出新行為。
  check(
    p0.found && !p0.isPre && near(p0.color, t['text-dim'], 1) && p1.found && !p1.isPre && near(p1.color, t['text-dim'], 1),
    `S1：503 期間 fg red 與 bg green 兩個獨立片段的計算後文字色都等於 --text-dim（實際 ${fmt(p0.color)}／${fmt(p1.color)}；獨立節點 ${!p0.isPre}／${!p1.isPre}）`
  );
  check(p1.found && p1.bg.a <= ALPHA_TOL, `S1：503 期間 bg green 片段沒有背景色（實際 ${fmt(p1.bg)}）`);
  const styledNodes = allLabels.filter((l) => dark.items[l].found && !dark.items[l].isPre).length;
  check(
    notDim.length === 0 && styledNodes >= 80,
    `S1：503 期間面板內所有 ${allLabels.length} 個片段文字一律 --text-dim 且不畫背景，且其中 >= 80 個是獨立片段節點（不符 ${notDim.length}：${JSON.stringify(notDim.slice(0, 3))}；獨立片段節點 ${styledNodes} 個）`
  );
  // 過期期間停留數個輪詢週期仍維持。
  await sleep(2200);
  const dark2 = await probe(cdp, pair);
  check(
    dark2.items[pair[0]].found && !dark2.items[pair[0]].isPre && near(dark2.items[pair[0]].color, t['text-dim'], 1) && near(dark2.items[pair[1]].color, t['text-dim'], 1) && dark2.items[pair[1]].bg.a <= ALPHA_TOL,
    'S1：過期期間經過數次重試後兩個片段仍為 --text-dim 且無背景'
  );
  // 恢復：不再攔截，真實端點回 200。
  icpt.set(null);
  await cdp.waitFor("!document.getElementById('output').classList.contains('is-stale')", 6000, 'S1：恢復回 200 後過期標示消失');
  await sleep(300);
  const back = await probe(cdp, pair);
  check(
    back.items[pair[0]].found && near(back.items[pair[0]].color, t.bad, 1),
    `S1：恢復後 fg red 片段回到 --bad 文字（實際 ${fmt(back.items[pair[0]].color)}）`
  );
  check(
    back.items[pair[1]].found && near(back.items[pair[1]].bg, t.ok, 0.2),
    `S1：恢復後 bg green 片段回到 --ok 20% 淡底（實際 ${fmt(back.items[pair[1]].bg)}）`
  );
}

// ---------------------------------------------------------------------------
// A8 不認得的顏色名稱（攔截回應塞入惡意名稱）
// ---------------------------------------------------------------------------

async function unknownColorCase(cdp, icpt) {
  log('--- A8 不認得的顏色名稱 ---');
  const evil = 'orange; background: red';
  const segments = [
    { text: 'unk-fg', fg: evil },
    { text: '\n' },
    { text: 'unk-bg', bg: evil },
    { text: '\n' },
    { text: 'ctl-red', fg: 'red' },
    { text: '\n' },
    { text: 'dim-red', fg: 'red', dim: true },
    { text: '\n' },
    { text: 'dim-only', dim: true },
    { text: '\n' },
  ];
  const text = segments.map((s) => s.text).join('');
  icpt.set({
    status: 200,
    body: JSON.stringify({ runtime: RUNTIME, pane_id: ANSI_PANE, format: 'text', text, segments, truncated: false }),
  });
  await cdp.waitFor("document.querySelector('#output .output-text').textContent.includes('unk-fg')", 6000, 'A8：面板顯示攔截回應的內容');
  await sleep(200);
  const snap = await probe(cdp, ['unk-fg', 'unk-bg', 'ctl-red', 'dim-red', 'dim-only']);
  const t = snap.tokens;
  const fg = snap.items['unk-fg'];
  const bg = snap.items['unk-bg'];
  const ctl = snap.items['ctl-red'];
  check(ctl.found && near(ctl.color, t.bad, 1), `A8 對照：同一份回應裡合法的 fg red 仍為 --bad（確認攔截內容有被畫出；實際 ${fmt(ctl.color)}）`);
  check(fg.found && near(fg.color, t.text, 1) && fg.bg.a <= ALPHA_TOL, `A8：fg 為不認得名稱的片段以預設文字顏色呈現、無背景（實際 ${fmt(fg.color)}／${fmt(fg.bg)}）`);
  check(bg.found && near(bg.color, t.text, 1) && bg.bg.a <= ALPHA_TOL, `A8：bg 為不認得名稱的片段以預設文字顏色呈現、無背景（實際 ${fmt(bg.color)}／${fmt(bg.bg)}）`);
  // A6（變暗）：有前景色的片段維持其前景色（樣本沒有 dim＋fg 的組合，借這份攔截回應驗）；沒有前景色的改用 --text-dim。
  const dimRed = snap.items['dim-red'];
  const dimOnly = snap.items['dim-only'];
  check(dimRed.found && near(dimRed.color, t.bad, 1), `A6：dim 加 fg red 的片段維持 --bad（不被 dim 改成 --text-dim；實際 ${fmt(dimRed.color)}）`);
  check(dimOnly.found && near(dimOnly.color, t['text-dim'], 1), `A6：只帶 dim 的片段為 --text-dim（實際 ${fmt(dimOnly.color)}）`);
  const exactBad = ['unk-fg', 'unk-bg', 'ctl-red', 'dim-red', 'dim-only'].filter((l) => !snap.items[l].exact);
  check(exactBad.length === 0, `A8：攔截回應的五個片段都是文字完全相等的獨立節點（不符 ${JSON.stringify(exactBad)}）`);
  const html8 = await cdp.eval(`(${pageHtmlProbe.toString()})()`);
  check(html8.styleAttrs === 0, `A8（design D7）：塞入不認得顏色名稱時，內容框內沒有任何元素帶 style 屬性（實際 ${html8.styleAttrs} 個）`);
  const leaks = await cdp.eval(`(${pageAttrLeaks.toString()})(${JSON.stringify(['orange', 'background'])})`);
  check(leaks.length === 0, `A8：面板上沒有任何元素的 class 或 style 含該字串（命中 ${leaks.length}：${JSON.stringify(leaks.slice(0, 3))}）`);
  icpt.set(null);
}

// ---------------------------------------------------------------------------
// M1 回應本體不是 JSON 物件（5.6 F2）、X1 segments 與 text 不一致（5.6 F3）：以 Fetch 攔截塞入
// ---------------------------------------------------------------------------

const PANEL_TEXT = "document.querySelector('#output .output-text').textContent";
const outputBody = (text, segments) =>
  JSON.stringify({ runtime: RUNTIME, pane_id: ANSI_PANE, format: 'text', text, segments, truncated: false });

// 200 但本體不是 JSON 物件（null、陣列、數字、字串）：前端要把它當成「不是合法 JSON」處理——不覆寫內容、
// 輪詢照節奏繼續，之後回合法本體就恢復（spec「輪詢與顯示」；改版前 null 會讓 applySuccess 拋出例外，
// onPollSettled 後面的 schedulePoll 不執行，輪詢永久停止）。
async function malformedBodyCase(cdp, icpt) {
  log('--- M1 回應本體不是 JSON 物件時輪詢不中止 ---');
  icpt.set({ status: 200, body: outputBody('m1-base', [{ text: 'm1-base' }]) });
  const base = await cdp.waitFor(`${PANEL_TEXT} === 'm1-base'`, 6000, 'M1：面板顯示攔截回應的基準內容');
  if (!base) {
    icpt.set(null);
    return;
  }
  const bodies = [['null', 'null'], ['陣列 []', '[]'], ['數字 7', '7'], ['字串 "x"', '"x"']];
  for (const [idx, [name, raw]] of bodies.entries()) {
    icpt.set({ status: 200, body: raw });
    const before = icpt.hits();
    const start = Date.now();
    while (icpt.hits() - before < 3 && Date.now() - start < 8000) await sleep(100);
    const polled = icpt.hits() - before;
    check(polled >= 3, `M1：回應本體為 ${name} 時，前端仍在輪詢（8 秒內至少 3 次後續請求；實際 ${polled} 次）`);
    const kept = await cdp.eval(PANEL_TEXT);
    check(kept === 'm1-base', `M1：回應本體為 ${name} 時，面板內容維持原樣、不被清空（實際 ${JSON.stringify(kept)}）`);
    const next = `m1-after-${idx}`;
    icpt.set({ status: 200, body: outputBody(next, [{ text: next }]) });
    const back = await cdp.waitFor(`${PANEL_TEXT} === ${JSON.stringify(next)}`, 6000, `M1：${name} 之後回合法本體，面板恢復顯示新內容`);
    if (!back) break;
    icpt.set({ status: 200, body: outputBody('m1-base', [{ text: 'm1-base' }]) });
    await cdp.waitFor(`${PANEL_TEXT} === 'm1-base'`, 6000, 'M1：回到基準內容');
  }
  icpt.set(null);
}

// 面板 textContent 恆等於回應的 text（spec「輪詢與顯示」）：segments 陣列中 text 不是字串的元素被略過、
// 或可用片段串接後與 text 不一致（端點違反自身不變式）時，前端退回 [{ text }]（design D7）。
async function textMismatchCase(cdp, icpt) {
  log('--- X1 segments 與 text 不一致時 textContent 仍等於 text ---');
  const cases = [
    {
      name: '陣列中有 text 非字串的元素，其餘片段串接為 abcd 但 text 為 abXcd',
      text: 'x1-abXcd',
      segments: [{ text: 'x1-ab', fg: 'red' }, { text: 5, fg: 'green' }, { text: 'cd' }],
      fallback: true,
    },
    {
      name: '片段串接（zzz）與 text 完全不同',
      text: 'x1-hello world',
      segments: [{ text: 'x1-zzz', fg: 'red' }],
      fallback: true,
    },
    {
      name: '對照：片段串接等於 text 時照片段上色',
      text: 'x1-ok red',
      segments: [{ text: 'x1-ok', fg: 'red' }, { text: ' red' }],
      fallback: false,
      styled: 'x1-ok',
    },
    {
      name: '對照：略過 text 非字串的元素後串接仍等於 text 時照片段上色',
      text: 'x1-keep green',
      segments: [{ text: 'x1-keep' }, { text: 7, fg: 'red' }, { text: ' green', fg: 'green' }],
      fallback: false,
      styled: ' green',
    },
  ];
  for (const c of cases) {
    icpt.set({ status: 200, body: outputBody(c.text, c.segments) });
    const shown = await cdp.waitFor(`${PANEL_TEXT} === ${JSON.stringify(c.text)}`, 6000, `X1：${c.name}：面板 textContent 等於回應 text`);
    if (!shown) continue;
    const spans = await cdp.eval("document.querySelectorAll('#output .output-text span').length");
    if (c.fallback) {
      check(spans === 0, `X1：${c.name}：退回 [{ text }]，沒有任何樣式 span（實際 ${spans} 個）`);
    } else {
      const snap = await probe(cdp, [c.styled]);
      const it = snap.items[c.styled];
      check(it.found && it.exact && !it.isPre && spans >= 1, `X1：${c.name}：仍依片段建出獨立的樣式節點（span ${spans} 個）`);
    }
  }
  icpt.set(null);
}

// ---------------------------------------------------------------------------
// P2 只有顏色改變也會重畫（ansi-flip）
// ---------------------------------------------------------------------------

async function flipCase(cdp, requests) {
  log('--- P2 只有顏色改變也會重畫（ansi-flip）---');
  await selectPane(cdp, FLIP_PANE);
  const shown = await cdp.waitFor("(() => { const t = document.querySelector('#output .output-text'); return !!t && t.textContent.startsWith('status'); })()", 5000, 'P2：面板顯示 ansi-flip 的 status');
  if (!shown) return;
  const reqBefore = requests.length;
  const samples = [];
  const start = Date.now();
  while (Date.now() - start < 5000) {
    const s = await probe(cdp, ['status']);
    const it = s.items.status;
    samples.push({
      t: Date.now() - start,
      text: s.panelText,
      color: it.found ? it.color : null,
      tokens: s.tokens,
    });
    await sleep(100);
  }
  const t = samples[0].tokens;
  const kind = (c) => (!c ? 'none' : near(c, t.bad, 1) ? 'bad' : near(c, t.ok, 1) ? 'ok' : 'other');
  const kinds = samples.map((s) => kind(s.color));
  const firstBad = kinds.indexOf('bad');
  const firstOk = kinds.indexOf('ok');
  check(samples.every((s) => s.text === 'status\n'), `P2：期間面板 textContent 一律等於 status＋換行（text 不變；不同值 ${JSON.stringify([...new Set(samples.map((s) => s.text))])}）`);
  check(firstBad >= 0 && firstOk >= 0, `P2：5 秒內 status 的計算後顏色曾為 --bad 也曾為 --ok（取樣 ${kinds.length} 次；種類 ${JSON.stringify([...new Set(kinds)])}）`);
  if (firstBad >= 0 && firstOk >= 0) {
    const first = Math.min(firstBad, firstOk);
    const other = firstBad < firstOk ? firstOk : firstBad;
    const lag = samples[other].t - samples[first].t;
    check(lag <= 3000, `P2：從第一種顏色到另一種顏色出現相隔 ${lag} ms（<= 3000）`);
  }
  let flips = 0;
  for (let i = 1; i < kinds.length; i++) if (kinds[i] !== kinds[i - 1] && kinds[i] !== 'none' && kinds[i - 1] !== 'none') flips += 1;
  check(flips >= 3, `P2：5 秒內顏色至少來回切換 3 次（輪詢每秒一次；實際 ${flips} 次，輸出請求 ${requests.length - reqBefore} 次）`);
}

// ---------------------------------------------------------------------------
// 截圖（live-output-color task 5.4；--screenshots）
// ---------------------------------------------------------------------------

// 截圖去識別化：作法同 docs/research/2026-10-01/ui-fixes-check.js 的 maskExpression。ui_preview 的 fixture 把
// pane cwd 放在真實 %TEMP% 底下，畫面會出現 `C:\Users\<真實使用者名稱>\AppData\Local\Temp\…`。截圖前在頁面裝
// MutationObserver，把文字節點中 `Users\` 之後的路徑段與真實使用者名稱、主機名稱換成 `<user>`；背景重畫
// 一直重建節點，所以要持續替換。名稱在執行時由 os 取得，不寫進 repo。
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

// 截圖前確認頁面文字（textContent，含被 CSS 截斷的部分）不含真實使用者名稱或主機名稱。
async function assertMasked(cdp, width) {
  const names = [os.userInfo().username, os.hostname()].filter(Boolean);
  const text = await cdp.eval('document.body.textContent');
  const hit = names.filter((n) => text.toLowerCase().includes(n.toLowerCase()));
  check(hit.length === 0, `截圖 ${width}：截圖前頁面文字不含真實使用者名稱與主機名稱（命中 ${hit.length} 個）`);
  return hit.length === 0;
}

// 選 ansi pane，把有色格線捲進可視範圍，三種 viewport 寬各截一張。
async function screenshotCase(cdp) {
  log('--- 截圖 ansi 模式（1536／1100／700）---');
  await selectPane(cdp, ANSI_PANE);
  await cdp.waitFor(ansiRendered, 8000, '截圖：ansi 樣本已畫在 Live Output 面板');
  await cdp.eval(maskExpression());
  for (const [w, h] of [[1536, 1024], [1100, 900], [700, 900]]) {
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
    await sleep(500);
    // 內容框與其祖先捲到頂（格線在最前面），再把面板捲進頁面可視範圍。
    await cdp.eval(`(() => {
      const pre = document.querySelector('#output .output-text');
      for (let n = pre; n && n !== document.body; n = n.parentElement) n.scrollTop = 0;
      document.getElementById('output').scrollIntoView({ block: 'start' });
      return true;
    })()`);
    await sleep(300);
    if (!(await assertMasked(cdp, w))) continue;
    const shot = await cdp.send('Page.captureScreenshot', { format: 'png' });
    const file = path.join(__dirname, `output-color-${w}.png`);
    fs.writeFileSync(file, Buffer.from(shot.result.data, 'base64'));
    log(`wrote ${file}`);
  }
  await cdp.send('Emulation.clearDeviceMetricsOverride');
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

async function partColor() {
  log('=== Live Output 依樣式上色（ui_preview，ansi／ansi-flip 模式）===');
  if (!fs.existsSync(UI_PREVIEW_EXE)) {
    throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
  }
  const port = pickPort(7770);
  if (port !== 7770) log(`7770 已被占用，改用埠 ${port}`);
  const cdpPort = pickPort(18810, [port]);

  const requests = [];
  let chrome = null;
  let server = null;
  try {
    server = spawn(UI_PREVIEW_EXE, [], {
      stdio: ['ignore', 'pipe', 'ignore'],
      windowsHide: true,
      env: {
        ...process.env,
        COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`,
        COCKPIT_PREVIEW_PUSH_MS: '500',
        COCKPIT_PREVIEW_OUTPUT_MODES: `${ANSI_PANE}=ansi;${FLIP_PANE}=ansi-flip`,
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
        const m = /^output-request (\S+) (\S+)$/.exec(line);
        if (m) requests.push({ runtime: m[1], pane: m[2], at: Date.now() });
      }
    });
    let up = false;
    for (let i = 0; i < 50 && !up; i++) {
      try {
        up = (await fetch(`http://127.0.0.1:${port}/api/state`)).ok;
      } catch {
        // 還沒起來。
      }
      if (!up) await sleep(200);
    }
    check(up, `ui_preview 應該在 10 秒內開始回應（port ${port}）`);
    if (!up) throw new Error('ui_preview 沒有起來');

    const url = `http://127.0.0.1:${port}/`;
    chrome = await startChrome(cdpPort, url, 'output-color');
    const { cdp } = chrome;
    await cdp.waitFor("typeof window.liveOutput === 'object' && document.querySelectorAll('.pane-row[data-action=\"select-pane\"]').length >= 2", 8000, '頁面載入完成並畫出可選的 pane 列');
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 });

    await selectPane(cdp, ANSI_PANE);
    await cdp.waitFor(ansiRendered, 8000, 'ansi 樣本已畫在 Live Output 面板');
    await sleep(300);

    await colorCase(cdp);
    await textContentCase(cdp);
    await noRewriteCase(cdp, requests);
    const icpt = makeIntercept(cdp);
    await staleCase(cdp, icpt);
    await unknownColorCase(cdp, icpt);
    await malformedBodyCase(cdp, icpt);
    await textMismatchCase(cdp, icpt);
    await icpt.disable();
    await flipCase(cdp, requests);
    if (process.argv.includes('--screenshots')) await screenshotCase(cdp);
  } finally {
    await stopChrome(chrome, 'chrome-output-color');
    killTree(server, 'ui_preview');
    await sleep(300);
    check(!isPortListening(port), `port ${port} 應該不再有 LISTENING 的行程`);
  }
}

async function main() {
  if (!fs.existsSync(CHROME)) {
    throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
  }
  try {
    await partColor();
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
