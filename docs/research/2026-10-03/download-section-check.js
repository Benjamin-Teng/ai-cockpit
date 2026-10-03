// release-packaging：宣傳頁「下載」區塊驗收（spec「宣傳頁下載區塊」三個情境＋i18n 鍵對應）。
// repo 沒有 playwright 依賴，沿用既有驗收腳本的做法：headless Chrome＋原生 CDP（Node 22 內建 WebSocket），
// 以 CDP 的 Fetch 域攔截 api.github.com 的請求（等同 Playwright 的 page.route）。
// 用法（repo 根）：
//   node docs/research/2026-10-03/download-section-check.js
// 環境變數 COCKPIT_CHROME 可指定瀏覽器執行檔（預設 Windows 的 Chrome）。
// 只終止本腳本自己 spawn 的 chrome（依 PID）。任一項 FAIL 以非 0 結束。
const os = require('node:os');
const http = require('node:http');
const vm = require('node:vm');
const path = require('node:path');
const fs = require('node:fs');
const { spawn, spawnSync } = require('node:child_process');

const REPO = path.resolve(__dirname, '..', '..', '..');
const SITE = path.join(REPO, 'site');
const CHROME = process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
const CDP_PORT = 18891;
const RELEASES = 'https://github.com/Benjamin-Teng/ai-cockpit/releases/latest';
const FAKE_SETUP = 'https://example.invalid/dl/ai-cockpit-0.1.0-x64-setup.exe';
const FAKE_ZIP = 'https://example.invalid/dl/ai-cockpit-0.1.0-x64.zip';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let failures = 0;
function check(name, ok, detail = '') {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail && !ok ? `  -> ${detail}` : ''}`);
}

// ---------------- 靜態伺服器：提供 site/ ----------------
const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.png': 'image/png' };
function serve() {
  return new Promise((resolve) => {
    const srv = http.createServer((req, res) => {
      const p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
      const file = path.join(SITE, p === '/' ? 'index.html' : p);
      if (!file.startsWith(SITE) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
        res.writeHead(404).end();
        return;
      }
      res.writeHead(200, { 'content-type': MIME[path.extname(file)] || 'application/octet-stream' });
      fs.createReadStream(file).pipe(res);
    });
    srv.listen(0, '127.0.0.1', () => resolve(srv));
  });
}

// ---------------- 最小 CDP client（browser 層 ws＋flatten session） ----------------
class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.handlers = new Map(); // sessionId -> fn(method, params)
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        const { res, rej } = this.pending.get(m.id);
        this.pending.delete(m.id);
        m.error ? rej(new Error(`${m.error.message}`)) : res(m.result);
      } else if (m.method && m.sessionId && this.handlers.has(m.sessionId)) {
        this.handlers.get(m.sessionId)(m.method, m.params);
      }
    };
  }
  send(method, params = {}, sessionId) {
    return new Promise((res, rej) => {
      const id = ++this.id;
      this.pending.set(id, { res, rej });
      this.ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }
}

// 一個情境一個乾淨分頁：攔截 api.github.com、收集 pageerror／console.warn
async function openPage(cdp, base, { lang, route, width = 1280, height = 900 }) {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const s = (m, p) => cdp.send(m, p, sessionId);
  const page = { sessionId, pageErrors: [], warns: [], apiCalls: [], send: s, targetId };
  cdp.handlers.set(sessionId, (method, params) => {
    if (method === 'Runtime.exceptionThrown') page.pageErrors.push(params.exceptionDetails.text + ' ' + (params.exceptionDetails.exception?.description || ''));
    if (method === 'Runtime.consoleAPICalled' && params.type === 'warning') page.warns.push(params.args.map((a) => a.value ?? a.description).join(' '));
    if (method === 'Fetch.requestPaused') {
      page.apiCalls.push(params.request.url);
      const reqId = params.requestId;
      if (route.kind === 'abort') {
        s('Fetch.failRequest', { requestId: reqId, errorReason: 'Failed' });
      } else {
        s('Fetch.fulfillRequest', {
          requestId: reqId,
          responseCode: route.status,
          responseHeaders: [
            { name: 'Content-Type', value: 'application/json; charset=utf-8' },
            { name: 'Access-Control-Allow-Origin', value: '*' },
          ],
          body: Buffer.from(JSON.stringify(route.body ?? {})).toString('base64'),
        });
      }
    }
  });
  await s('Page.enable');
  await s('Runtime.enable');
  await s('Fetch.enable', { patterns: [{ urlPattern: 'https://api.github.com/*' }] });
  await s('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
  // 語言：在任何頁面腳本之前先寫入 localStorage，等同使用者按過切換
  await s('Page.addScriptToEvaluateOnNewDocument', { source: `try{localStorage.setItem('cockpit.site.lang',${JSON.stringify(lang)})}catch(e){}` });
  page.ev = async (expression) => {
    const r = await s('Runtime.evaluate', { expression, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
    return r.result.value;
  };
  page.goto = async () => {
    await s('Page.navigate', { url: base + '/' });
    for (let i = 0; i < 100 && (await page.ev('document.readyState')) !== 'complete'; i++) await sleep(100);
  };
  page.waitFor = async (expr, ms = 4000) => {
    for (let t = 0; t < ms; t += 100) {
      if (await page.ev(expr)) return true;
      await sleep(100);
    }
    return false;
  };
  page.close = () => cdp.send('Target.closeTarget', { targetId });
  return page;
}

const STATE = `(() => {
  const bar = document.getElementById('dl-version');
  const r = bar.getBoundingClientRect();
  return {
    visible: !bar.hidden && r.width > 0 && r.height > 0,
    text: bar.textContent,
    setup: document.getElementById('dl-setup').href,
    zip: document.getElementById('dl-zip').href,
  };
})()`;

// ---------------- 情境 ----------------
const release = (assets) => ({ tag_name: 'v0.1.0', published_at: '2026-10-05T03:00:00Z', assets });
const asset = (name, url) => ({ name, browser_download_url: url });
const CASES = [
  { id: 'a', title: '200 且資產齊全', route: { kind: 'json', status: 200, body: release([asset('ai-cockpit-0.1.0-x64-setup.exe', FAKE_SETUP), asset('ai-cockpit-0.1.0-x64.zip', FAKE_ZIP), asset('SHA256SUMS.txt', 'https://example.invalid/sums')]) }, ok: true },
  { id: 'b', title: '404', route: { kind: 'json', status: 404, body: { message: 'Not Found' } }, ok: false },
  { id: 'c', title: '網路錯誤（abort）', route: { kind: 'abort' }, ok: false },
  { id: 'd', title: '200 但資產名稱不符', route: { kind: 'json', status: 200, body: release([asset('source.zip', 'https://example.invalid/source.zip')]) }, ok: false },
  { id: 'e', title: '200 但只有安裝檔、缺 zip（不可只換一個）', route: { kind: 'json', status: 200, body: release([asset('ai-cockpit-0.1.0-x64-setup.exe', FAKE_SETUP)]) }, ok: false },
  { id: 'f', title: '200 但缺 tag_name', route: { kind: 'json', status: 200, body: { published_at: '2026-10-05T03:00:00Z', assets: [asset('ai-cockpit-0.1.0-x64-setup.exe', FAKE_SETUP), asset('ai-cockpit-0.1.0-x64.zip', FAKE_ZIP)] } }, ok: false },
];

async function runCase(cdp, base, c) {
  const page = await openPage(cdp, base, { lang: 'en', route: c.route });
  await page.goto();
  // 成功情境等版本列出現；失敗情境等 API 請求被攔到並處理完
  if (c.ok) await page.waitFor(`!document.getElementById('dl-version').hidden`);
  else {
    await page.waitFor('true', 100);
    for (let i = 0; i < 50 && page.warns.length === 0; i++) await sleep(100);
  }
  const st = await page.ev(STATE);
  const tag = `(${c.id}) ${c.title}`;
  check(`${tag}: 頁面確實呼叫了 releases/latest`, page.apiCalls.some((u) => u === 'https://api.github.com/repos/Benjamin-Teng/ai-cockpit/releases/latest'), JSON.stringify(page.apiCalls));
  if (c.ok) {
    check(`${tag}: 版本列可見且含 v0.1.0 與 2026-10-05`, st.visible && st.text.includes('v0.1.0') && st.text.includes('2026-10-05'), JSON.stringify(st));
    check(`${tag}: 安裝檔按鈕 href 為假網址`, st.setup === FAKE_SETUP, st.setup);
    check(`${tag}: zip 按鈕 href 為假網址`, st.zip === FAKE_ZIP, st.zip);
    check(`${tag}: 沒有 console.warn`, page.warns.length === 0, page.warns.join(' | '));
  } else {
    check(`${tag}: 版本列不可見且為空`, !st.visible && st.text === '', JSON.stringify(st));
    check(`${tag}: 兩按鈕 href 仍為 releases/latest`, st.setup === RELEASES && st.zip === RELEASES, `${st.setup} ${st.zip}`);
    check(`${tag}: 有 console.warn 記錄`, page.warns.length >= 1);
  }
  check(`${tag}: 沒有未捕捉例外（pageerror = 0）`, page.pageErrors.length === 0, page.pageErrors.join(' | '));
  check(`${tag}: 頁面其餘部分照常（hero 標題與 #start 存在）`, await page.ev(`!!document.querySelector('.hero h1') && !!document.getElementById('start')`));
  return page;
}

const SHOT = (cdp, page, file) =>
  (async () => {
    const rect = await page.ev(`(() => { const r = document.getElementById('download').getBoundingClientRect(); return { x: 0, y: r.top + scrollY, width: 1280, height: r.height }; })()`);
    const shot = await page.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true, clip: { ...rect, scale: 1 } });
    fs.writeFileSync(file, Buffer.from(shot.data, 'base64'));
    return file;
  })();

// 截圖會進 repo：頁面文字不得含真實使用者名稱或主機名稱
async function noPersonalInfo(page, label) {
  const names = [os.userInfo().username, os.hostname()].filter(Boolean);
  const text = (await page.ev('document.documentElement.outerHTML')).toLowerCase();
  const hit = names.filter((n) => text.includes(n.toLowerCase()));
  check(`${label}: 頁面不含本機使用者／主機名稱`, hit.length === 0, `${hit.length} 個命中`);
}

// ---------------- i18n 鍵對應 ----------------
function i18nKeys() {
  const html = fs.readFileSync(path.join(SITE, 'index.html'), 'utf8');
  const htmlKeys = new Set([...html.matchAll(/data-i18n="([^"]+)"/g)].map((m) => m[1]));
  const sandbox = { window: {} };
  vm.runInNewContext(fs.readFileSync(path.join(SITE, 'i18n.js'), 'utf8'), sandbox);
  const dictKeys = new Set(Object.keys(sandbox.window.COCKPIT_I18N));
  const onlyHtml = [...htmlKeys].filter((k) => !dictKeys.has(k));
  const onlyDict = [...dictKeys].filter((k) => !htmlKeys.has(k));
  console.log(`i18n: index.html ${htmlKeys.size} 個鍵、i18n.js ${dictKeys.size} 個鍵`);
  check('i18n: index.html 有、字典沒有的鍵 = 空集合', onlyHtml.length === 0, onlyHtml.join(', '));
  check('i18n: 字典有、index.html 沒有的鍵 = 空集合', onlyDict.length === 0, onlyDict.join(', '));
}

async function main() {
  i18nKeys();
  const srv = await serve();
  const base = `http://127.0.0.1:${srv.address().port}`;
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-dl-check-'));
  const chrome = spawn(
    CHROME,
    ['--headless=new', '--disable-gpu', '--no-first-run', `--remote-debugging-port=${CDP_PORT}`, '--remote-allow-origins=*', `--user-data-dir=${udd}`, 'about:blank'],
    { stdio: 'ignore', windowsHide: true }
  );
  try {
    let wsUrl = null;
    for (let i = 0; i < 100 && !wsUrl; i++) {
      try {
        wsUrl = (await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/version`)).json()).webSocketDebuggerUrl;
      } catch {
        await sleep(200);
      }
    }
    if (!wsUrl) throw new Error('Chrome 的 CDP 沒有起來');
    const ws = new WebSocket(wsUrl);
    await new Promise((res, rej) => ((ws.onopen = res), (ws.onerror = rej)));
    const cdp = new Cdp(ws);

    for (const c of CASES) {
      const page = await runCase(cdp, base, c);
      if (c.id === 'a') {
        await noPersonalInfo(page, '(a) 英文');
        const en = await SHOT(cdp, page, path.join(__dirname, 'download-section-en.png'));
        console.log(`wrote ${en}`);
      }
      await page.close();
    }

    // (a) 繁中：與英文同一個假 release，語言以 localStorage 設定後載入
    const a = CASES[0];
    const zh = await openPage(cdp, base, { lang: 'zh', route: a.route });
    await zh.goto();
    await zh.waitFor(`!document.getElementById('dl-version').hidden`);
    const t = await zh.ev(`(() => {
      const txt = (sel) => (document.querySelector(sel) || {}).textContent || '';
      return {
        lang: document.documentElement.lang,
        h2: txt('#download h2'), intro: txt('#download .intro'),
        setup: txt('#dl-setup'), zip: txt('#dl-zip'),
        info: txt('#download .dl-info'), notes: [...document.querySelectorAll('#download .note')].map((n) => n.textContent).join(' '),
        bar: txt('#dl-version'), nav: txt('.nav-links a[href="#download"]'), cta: txt('.hero .ctas a[href="#download"]'),
        pending: document.documentElement.classList.contains('i18n-pending'),
      };
    })()`);
    const CJK = /[\u4e00-\u9fff]/;
    check('(a) 繁中: html lang = zh-Hant', t.lang === 'zh-Hant', t.lang);
    check('(a) 繁中: 下載區塊標題為中文', t.h2 === '下載', t.h2);
    check('(a) 繁中: 說明句為中文', CJK.test(t.intro), t.intro);
    check('(a) 繁中: 兩個按鈕為中文', CJK.test(t.setup) && CJK.test(t.zip), `${t.setup} / ${t.zip}`);
    check('(a) 繁中: 系統需求為中文', t.info.includes('系統需求') && t.info.includes('桌面啟動器需要 Chrome 或 Edge'), t.info);
    check('(a) 繁中: SmartScreen 說明為中文', t.info.includes('Windows 已保護您的電腦') && t.info.includes('其他資訊') && t.info.includes('仍要執行') && t.info.includes('SHA256SUMS.txt'), t.info);
    check('(a) 繁中: 安裝後說明與原始碼連結為中文', t.notes.includes('零設定') && t.notes.includes('從原始碼建置') && t.notes.includes('%LOCALAPPDATA%\\ai-cockpit\\cockpit.toml'), t.notes);
    check('(a) 繁中: 導覽與首屏 CTA 為中文', t.nav === '下載' && t.cta === '下載', `${t.nav} / ${t.cta}`);
    check('(a) 繁中: 版本列仍顯示 v0.1.0 與 2026-10-05', t.bar.includes('v0.1.0') && t.bar.includes('2026-10-05'), t.bar);
    const st = await zh.ev(STATE);
    check('(a) 繁中: 按鈕 href 不受語言切換影響', st.setup === FAKE_SETUP && st.zip === FAKE_ZIP, `${st.setup} ${st.zip}`);
    check('(a) 繁中: 沒有未捕捉例外', zh.pageErrors.length === 0, zh.pageErrors.join(' | '));
    // 實際點 #lang-toggle 切回英文再切回中文，版本列不得被清掉
    await zh.ev(`document.getElementById('lang-toggle').click(); true`);
    const afterEn = await zh.ev(`Array.from(document.getElementById('dl-version').children).map(c => c.textContent).join(' ') + '|' + document.querySelector('#download h2').textContent`);
    await zh.ev(`document.getElementById('lang-toggle').click(); true`);
    const afterZh = await zh.ev(`Array.from(document.getElementById('dl-version').children).map(c => c.textContent).join(' ') + '|' + document.querySelector('#download h2').textContent`);
    check('(a) 點 #lang-toggle 切英文後版本列仍在、標題為 Download', afterEn === 'v0.1.0 2026-10-05|Download', afterEn);
    check('(a) 再切回繁中後版本列仍在、標題為 下載', afterZh === 'v0.1.0 2026-10-05|下載', afterZh);
    await noPersonalInfo(zh, '(a) 繁中');
    const zhShot = await SHOT(cdp, zh, path.join(__dirname, 'download-section-zh.png'));
    console.log(`wrote ${zhShot}`);
    await zh.close();
    ws.close();
  } finally {
    if (chrome.exitCode === null) spawnSync('taskkill', ['/PID', String(chrome.pid), '/T', '/F']);
    srv.close();
    await sleep(500);
    fs.rmSync(udd, { recursive: true, force: true });
  }
}

main()
  .then(() => {
    console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAIL`);
    process.exitCode = failures === 0 ? 0 : 1;
  })
  .catch((e) => {
    console.error('FAIL (script error)', e);
    process.exitCode = 1;
  });
