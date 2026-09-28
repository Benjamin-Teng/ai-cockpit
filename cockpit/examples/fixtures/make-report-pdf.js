// file-review task 3.4（design D12 Open Questions）：產生 `review-repo/report.pdf`（3 頁、第 1 頁
// 含中文標題），供 PDF 檢視器與「中文 PDF」情境測試。
//
// 產生方式：用 headless Chrome 的 CDP `Page.printToPDF`，從下面內嵌的 HTML 產生。HTML 用 CSS
// `break-after: page` 分成 3 頁：第 1 頁標題是中文「檔案瀏覽驗收報告」，字型指定 Windows 內建的
// 「Microsoft JhengHei」；第 2、3 頁各有一段英文與中文內文。啟動 headless Chrome 與收尾的寫法
// 參考 `docs/research/2026-09-23/visual-check.js` 的 `startChrome`／`stopChrome`（Chrome 路徑、
// 臨時 user-data-dir、依 PID 收尾）。
//
// 用法（repo 根）：
//   node cockpit/examples/fixtures/make-report-pdf.js
//
// 產出 `cockpit/examples/fixtures/review-repo/report.pdf` 後，本腳本會自行驗證：數 PDF 物件裡
// `/Type /Page` 的個數是否為 3（頁數），以及是否含 `/FontFile2` 或 `/FontFile3`（字型已嵌入，
// 不是只靠系統字型替換，中文在沒裝這個字型的機器上開啟也不會變成方框）。
//
// 只用 Node 內建 API（`child_process`、`fs`、`path`、`node:net`）＋全域 `fetch`／`WebSocket`
// （Node 18+）；不新增 npm 套件。

const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');

const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';
const OUT_PATH = path.join(__dirname, 'review-repo', 'report.pdf');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 第 1 頁中文標題、第 2、3 頁各一段中英文內文；`break-after: page` 讓每個 `.page` 各自獨立成一頁
// （最後一個 `.page` 不需要，`printToPDF` 只印有內容的部分，多一個空白頁靠這個規則避免）。
const HTML = `<!doctype html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8" />
<title>檔案瀏覽驗收報告</title>
<style>
  * { box-sizing: border-box; }
  body {
    font-family: "Microsoft JhengHei", sans-serif;
    margin: 0;
    color: #111;
  }
  .page {
    padding: 48px;
    min-height: 90vh;
  }
  .page:not(:last-child) {
    break-after: page;
  }
  h1 {
    font-size: 28px;
    margin-bottom: 24px;
  }
  h2 {
    font-size: 20px;
    margin-bottom: 16px;
  }
  p {
    font-size: 14px;
    line-height: 1.8;
  }
</style>
</head>
<body>
  <section class="page">
    <h1>檔案瀏覽驗收報告</h1>
    <p>本文件為 file-review task 3.4 的假 repo fixture，用於測試 Cockpit 的 PDF 檢視器能否正確顯示
    中文字元、正確分頁（工具列顯示「1 / 3」）、以及逐頁連續捲動。</p>
    <p>Chinese Title Page for the File Review Acceptance Report Fixture.</p>
  </section>
  <section class="page">
    <h2>Page 2: English Content</h2>
    <p>This is the second page of the fixture PDF. It contains English text to verify that the
    PDF viewer renders Latin characters alongside embedded CJK glyphs without any layout issues.</p>
    <h2>第二頁：中文內文</h2>
    <p>這是第二頁的中文段落，測試中文字元在整份文件中持續正確顯示，而不只是第一頁的標題。</p>
  </section>
  <section class="page">
    <h2>Page 3: English Content</h2>
    <p>This is the third and final page of the fixture PDF, used to verify multi-page continuous
    scrolling and the page counter reaching the last page.</p>
    <h2>第三頁：中文內文</h2>
    <p>這是第三頁的中文段落。若三頁的中文標題與內文都能被辨識（非空白、非方框），代表字型已經
    正確嵌入 PDF，不依賴檢視端是否安裝這套字型。</p>
  </section>
</body>
</html>
`;

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
}

function isPortListening(port) {
  const r = spawnSync('netstat', ['-ano'], { encoding: 'utf8' });
  const needle = `127.0.0.1:${port} `;
  return (r.stdout || '').split('\n').some((line) => line.includes(needle) && line.includes('LISTENING'));
}

function pickPort(start) {
  let port = start;
  while (isPortListening(port)) port += 1;
  return port;
}

async function main() {
  if (!fs.existsSync(CHROME)) {
    throw new Error(`找不到 Chrome：${CHROME}（可用 COCKPIT_CHROME 環境變數覆寫路徑）`);
  }
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-make-report-pdf-'));
  const htmlPath = path.join(udd, 'report-source.html');
  fs.writeFileSync(htmlPath, HTML, 'utf8');
  const fileUrl = 'file:///' + htmlPath.replace(/\\/g, '/');
  const cdpPort = pickPort(9333);

  const chrome = spawn(
    CHROME,
    [
      '--headless=new',
      '--disable-gpu',
      '--no-first-run',
      `--remote-debugging-port=${cdpPort}`,
      '--remote-allow-origins=*',
      `--user-data-dir=${udd}`,
      'about:blank',
    ],
    { stdio: 'ignore', windowsHide: true }
  );

  let ws = null;
  try {
    let page = null;
    for (let i = 0; i < 100 && !page; i++) {
      try {
        const r = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
        const list = await r.json();
        page = list.find((t) => t.type === 'page');
      } catch {
        // CDP endpoint 還沒起來。
      }
      if (!page) await sleep(200);
    }
    if (!page) throw new Error('page target not found（輪詢逾時）');

    ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((res, rej) => {
      ws.onopen = res;
      ws.onerror = rej;
    });
    const cdp = new CDP(ws);

    await cdp.send('Page.enable');
    const navigated = new Promise((resolve) => {
      const handler = (e) => {
        const m = JSON.parse(e.data);
        if (m.method === 'Page.loadEventFired') {
          ws.removeEventListener('message', handler);
          resolve();
        }
      };
      ws.addEventListener('message', handler);
    });
    await cdp.send('Page.navigate', { url: fileUrl });
    await navigated;
    // 等字型與版面穩定（headless Chrome 首次載入本機字型偶爾略慢）。
    await sleep(300);

    const result = await cdp.send('Page.printToPDF', {
      printBackground: true,
      preferCSSPageSize: true,
      marginTop: 0,
      marginBottom: 0,
      marginLeft: 0,
      marginRight: 0,
    });
    if (!result.result || !result.result.data) {
      throw new Error(`Page.printToPDF 沒有回傳資料：${JSON.stringify(result)}`);
    }
    const pdfBytes = Buffer.from(result.result.data, 'base64');
    fs.mkdirSync(path.dirname(OUT_PATH), { recursive: true });
    fs.writeFileSync(OUT_PATH, pdfBytes);
    console.log(`已寫入 ${OUT_PATH}（${pdfBytes.length} bytes）`);

    // 驗證：頁數與字型嵌入。PDF 物件用 latin1 讀取足夠比對這幾個固定字串（不需要解析整個
    // PDF 結構）。
    const text = pdfBytes.toString('latin1');
    const pageCount = (text.match(/\/Type\s*\/Page[^s]/g) || []).length;
    const hasEmbeddedFont = /\/FontFile2/.test(text) || /\/FontFile3/.test(text);
    console.log(`頁數（/Type /Page 物件數）：${pageCount}`);
    console.log(`含嵌入字型（/FontFile2 或 /FontFile3）：${hasEmbeddedFont}`);
    if (pageCount !== 3) {
      throw new Error(`預期 3 頁，實際偵測到 ${pageCount} 頁`);
    }
    if (!hasEmbeddedFont) {
      throw new Error('PDF 沒有偵測到嵌入字型（/FontFile2 或 /FontFile3），中文可能無法在沒裝字型的機器上正確顯示');
    }
    console.log('驗證通過：3 頁、字型已嵌入。');
  } finally {
    if (ws) {
      try {
        ws.close();
      } catch {
        // 已斷線。
      }
    }
    if (chrome.exitCode === null && chrome.signalCode === null) {
      spawnSync('taskkill', ['/PID', String(chrome.pid), '/T', '/F'], { stdio: 'ignore' });
    }
    // Windows 上，Chrome 行程被強制終止後，它的使用者資料目錄裡有些檔案（例如 profile 的
    // Web Data）偶爾要再等一下下才會真的解鎖，第一次刪除可能撞上 EBUSY；重試幾次即可。
    let removed = false;
    for (let attempt = 0; attempt < 5 && !removed; attempt += 1) {
      try {
        fs.rmSync(udd, { recursive: true, force: true });
        removed = true;
      } catch {
        await sleep(200);
      }
    }
    if (!removed) {
      console.error(`清理暫存目錄失敗（重試 5 次仍鎖住）：${udd}`);
    }
  }
}

main().catch((e) => {
  console.error('失敗：', e);
  process.exitCode = 1;
});
