// i18n-check：介面語言（ui-language）驗收。各段說明與用法見同目錄 i18n-check.md。
//
// 權威：openspec/changes/ui-language/specs/ui-language/spec.md 與 design.md D7。
//
// 分段（design D7 的 ①～⑤；段代號用數字 1～5）：
//   ① 字典一致性（task 1.1）：解析 cockpit/assets/app/i18n.js（vm 沙箱載入），斷言繁中與英文兩份字典鍵集合相同、
//      每個鍵的具名佔位符集合相同，並檢查 t() 的代入與缺鍵行為、index.html 引用的每個 data-i18n 鍵都在字典裡。
//   ② 英文介面沒有繁中字典字串（task 2.1 起逐模組補；目前涵蓋 render.js／actions.js／output.js／files.js／viewers.js／git.js／notify.js 的畫面）：以 cockpit.lang=en
//      開 ui_preview，走頂列與燈號、左欄 Project 清單、Factory Floor（改綁模式 banner、空狀態、警告數、歧義綁定、未宣告 task）、
//      runtime 卡片（標籤）、最近事件與底列通道、錯誤 banner（409 與請求沒有完成）、Live Output 各狀態（空／截斷／請求失敗／
//      非 JSON 的 502／pane 已不存在／前端逾時），每個畫面收集全部文字節點與 title／aria-label／placeholder，依下面的規則比對
//      繁中字典值，並檢查 #app／#output／檔案樹／檔案分頁沒有殘留的 CJK 文字（使用者資料與後端原文欄位除外）；各處的英文字串另逐字斷言。
//      task 2.2 加的檔案畫面（另起一個推送間隔拉長的 ui_preview）：左欄檔案樹（空狀態、正在讀取根目錄、資料夾讀取中／空資料夾／
//      「還有 N 項未顯示」單複數／讀取失敗、重新整理、根目錄查詢的 16 個錯誤碼）、檔案分頁（分頁 title、關閉鈕 aria-label、尚未讀取、
//      正在讀取、讀取於、在 VS Code 開啟、過期與 7 個檔案分頁錯誤碼）、各種檢視器（Markdown 的錨點 aria-label／被擋的連結與圖片、
//      HTML iframe title、PDF 工具列與頁碼、PDF 無法解析、純文字檔案太大（中繼資料與實際內容兩條路，大小已知與未知）、尚未實作的
//      檢視器、二進位與圖片的不支援預覽）。fixture 沒有的邊界狀態靠頁面內的假 fetch 提供（FAKE_INSTALL／H／setRules）。
//      task 2.3 加的 git 畫面（同樣另起一個推送間隔拉長的 ui_preview，另有一份只跑這段的開發用旗標 --git-only）：左欄「變更」面板
//      （四個分組標題與筆數、衝突組、分離 HEAD、清單被截斷、沒有變更、正在讀取變更、不是 git repo、根目錄查詢與 git 狀態各錯誤碼、過期）、
//      diff 分頁（工作區／已暫存／空 的版本標示、分頁 title 與關閉鈕、二進位／只有權限改變／子模組／兩側相同／省略 N 行單複數／讀取中、
//      過期與錯誤）、Git Graph 分頁（工具列、分支篩選三組、搜尋筆數單複數、詳情的各標籤與按鈕、複製回饋、比較基準與比較詳情、
//      變更檔案單複數、詳情讀取中／錯誤／截斷／父 commit 不在已載入範圍、分支已變更提示、清單讀取中／錯誤／空／已達上限）、某版本檔案分頁
//      （暫存區版本標示、分頁 title、開啟目前版本、過期、讀取中、尚未實作的檢視器、錯誤）。fixture 有的（衝突組、merge commit、三種 ref、
//      二百筆以上 commit）用真的，沒有的靠頁面內的假 fetch（含 5000 筆的假 log 回應）。
//      task 2.4 加的通知畫面（同一個 ui_preview；`--notify-only` 是只跑這一段的開發用旗標，搭配 --only=2）：鈴鐺的 aria-label／title、
//      設定面板（標題、前景說明、四個事件名稱與說明）、權限四種狀態（尚未決定／已允許／已封鎖／不支援，另按下「允許通知」後轉為已允許）
//      與桌面通知的標題與內文（單一事件：agent blocked、agent done 帶多個綁定名稱、task failed、task completed；超過 3 件合併後的標題與
//      內文）。權限與桌面通知以頁面內換掉 window.Notification 的記錄器提供（同 notify-check.js 的做法）；狀態轉換直接呼叫
//      cockpitNotify.observe（先餵基準、再餵改過的投影）。面板節點在 body 底下、#app 之外，殘留 CJK 檢查範圍因此加上 #notify-panel。
//      task 3.3 加的後端訊息代碼：連線原因、protocol 警告、project 警告不再排除，英文介面 WSL 卡片原因為英文且含 Ubuntu-24.04；錯誤 banner 以假 fetch 回
//      帶 code 的 409 驗（代碼翻譯、參數原樣、未知代碼與沒有 code 退回原文）；Live Output 的 503／504 以代碼驗；同步餵改過的投影驗 reason_msg／
//      protocol_warning_msg／warning_msgs 的未知代碼、欄位不存在、陣列較短時退回原文。最近事件 drift 說明的五個狀態漂移代碼（workspace／tab／pane
//      不存在、runtime 未登記、payload 無法解析）英文翻譯且無 CJK，未知代碼與缺 detail_msg 退回原文。
//      同一套流程再以繁中跑一次，逐字比對「改動前」的繁中字串（獨立於字典），確認繁中介面完全沒變。
//   ③ 語言決定（task 1.1）：spec「介面語言的決定」四個 scenario 以 headless Chrome＋CDP（Emulation.setTimezoneOverride、
//      Emulation.setUserAgentOverride 的 acceptLanguage）端到端驗；另在頁面內直接呼叫純函式
//      cockpitI18n.resolveLang() 驗邊界（Chrome 的 --lang 會改寫 zh-SG／zh-HK 等值，端到端測不到）；
//      localStorage 存取丟例外時視為沒有該項資訊。
//   ④ 切換按鈕與兩個視窗同步（task 1.2）：頂列按鈕的位置、文字、lang、可及名稱，滑鼠點擊與鍵盤 Enter 切換
//      （cockpit.lang、重新載入、<html lang>、按鈕文字），鍵盤焦點可見；同一個 Chrome 兩個分頁（同源）其中一個切換、
//      另一個跟著重新載入（無關的 storage 鍵不重載）；localStorage 的 setItem 丟例外時（CDP
//      Page.addScriptToEvaluateOnNewDocument 覆寫 Storage.prototype.setItem）按鈕停用並有 title、點了不動作。
//      另外 ① 以沙箱驗 canPersist、setLang 寫入失敗不重載、storage 事件的重載判斷。
//   ⑤ 英文介面截圖（task 5.1）：cockpit.lang=en，1536×1024／1100×900／700×900 三種寬度各拍五個畫面（預設儀表板選了 pane 且 Live Output 顯示中、
//      通知設定面板、左欄 Changes、Git Graph 分頁、Markdown 檔案分頁），存成同目錄 i18n-en-<畫面>-<寬度>.png，交 frontend-design 審核。
//      去識別化：拍前裝 MutationObserver 把 `Users\` 之後的路徑段與真實使用者名稱、主機名稱換成 `<user>`，並斷言文字與屬性都不含真名；
//      PNG 仍須逐張看圖確認（見 docs/research/2026-10-02/deid-check.md）。
//
// ②的比對規則（SDD ledger 裁決，後續 task 實作時照辦）：
//   - 取英文介面（cockpit.lang=en）頁面上所有文字節點的 textContent，以及 title／aria-label／placeholder 屬性值。
//   - 把繁中字典每個值的佔位符 `{name}` 拿掉，得到「固定片段」。
//   - 比對方式有二：(a) 整段 trim 後的節點文字／屬性值，與「去掉佔位符後的整個繁中字典值」完全相等者算命中；
//     (b) 固定片段含 ≥ 4 個連續 CJK 字元時，節點文字／屬性值只要「包含」該片段也算命中（涵蓋拼接與夾帶資料的句子）。
//     短於 4 個 CJK 字元的片段只做 (a)，避免「檔案」「變更」這類兩字詞誤報。
//   - 排除使用者資料節點（project／stage／task／workstream 名稱、檔案內容、pane 輸出、git 資料、HERDR 原文），
//     例如範例資料裡的中文 task 名稱要照原樣顯示、不算命中。task 3.3 起後端訊息（連線 reason、protocol 警告、project 警告、
//     錯誤 banner 與 Live Output 的原因）依代碼翻譯，不再排除。排除清單見腳本的 USER_DATA_SEL。
//   - 實作補充（比上面的規則更嚴，不是放寬）：(a') 整段文字符合「佔位符當萬用字元」的繁中範本也算命中（抓 `警告 3`、`歧義（2）`
//     這種帶資料、短於 4 個 CJK 字元的句子）；繁中值裡沒有任何 CJK 字元的（`HTTP {status}`、`EN`）不可能是繁中字串，略過；
//     另對 #app／#output 加一條「沒有殘留 CJK 文字」，抓還沒進字典的硬編碼中文。
//   - task 2.4 之後所有模組都已進字典，不再有「待處理模組」的排除清單（原 PENDING_MODULE_SEL 已刪除）。
//
// 輸出：每條斷言一行 PASS／FAIL；「前置：」開頭的是環境與既有行為的前提；結尾列斷言數，最後一行
// `RESULT: PASS` 或 `RESULT: FAIL (N)`，有失敗時結束碼非 0。
//
// 用法（repo 根，③ 需先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-10-03/i18n-check.js [--only=1,3]
// 清理：只終止本腳本自己 spawn 的 ui_preview.exe／chrome.exe（依 PID），埠被占用就往上找空埠（從 7780 起，避開 7770）。
const os = require('node:os');
const vm = require('node:vm');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const I18N_JS = path.join(REPO, 'cockpit', 'assets', 'app', 'i18n.js');
const INDEX_HTML = path.join(REPO, 'cockpit', 'assets', 'index.html');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

// ---------------------------------------------------------------------------
// 斷言與記錄
// ---------------------------------------------------------------------------

const failures = [];
const tally = { pre: { pass: 0, fail: 0 }, spec: { pass: 0, fail: 0 } };
// spec 斷言（本 change 的新行為）。
function check(cond, label) {
  const ok = !!cond;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${label}`);
  tally.spec[ok ? 'pass' : 'fail'] += 1;
  if (!ok) failures.push(label);
  return ok;
}
// 前置（環境、fixture、既有行為）：失敗同樣算 FAIL，但分開計數。
function pre(cond, label) {
  const ok = !!cond;
  console.log(`${ok ? 'PASS' : 'FAIL'} 前置：${label}`);
  tally.pre[ok ? 'pass' : 'fail'] += 1;
  if (!ok) failures.push(`前置：${label}`);
  return ok;
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const J = (v) => JSON.stringify(v);
const sameSet = (a, b) => a.length === b.length && a.every((x) => b.includes(x));

// ---------------------------------------------------------------------------
// 行程與埠（同 notify-check.js）
// ---------------------------------------------------------------------------

function pidStillRunning(pid) {
  const r = spawnSync('tasklist', ['/FI', `PID eq ${pid}`, '/NH'], { encoding: 'utf8' });
  return typeof r.stdout === 'string' && r.stdout.includes(String(pid));
}

function killTree(child, label) {
  if (!child || child.exitCode !== null) return;
  spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8' });
  pre(!pidStillRunning(child.pid), `${label} PID ${child.pid} 已終止（tasklist 查無此 PID）`);
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

// ---------------------------------------------------------------------------
// CDP
// ---------------------------------------------------------------------------

class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.exceptions = []; // Runtime.exceptionThrown（未捕捉例外與未處理的 rejection）
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id && this.pending.has(m.id)) {
        this.pending.get(m.id)(m);
        this.pending.delete(m.id);
      } else if (m.method === 'Runtime.exceptionThrown') {
        const d = m.params.exceptionDetails || {};
        this.exceptions.push({
          at: Date.now(),
          text: `${d.text || ''} ${(d.exception && d.exception.description) || ''}`.trim().slice(0, 300),
          url: d.url || '',
        });
      }
    };
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((res) => {
      this.pending.set(id, res);
      this.ws.send(J({ id, method, params }));
    });
  }
  async eval(expression) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.result && r.result.exceptionDetails) {
      throw new Error(`頁面內例外：${J(r.result.exceptionDetails).slice(0, 400)}`);
    }
    return r.result && r.result.result ? r.result.result.value : undefined;
  }
  // 在頁面內呼叫一個（可為 async 的）函式；函式本體以 toString 序列化，參數以 JSON 傳入。
  fn(f, ...args) {
    return this.eval(`(${f.toString()})(...${J(args)})`);
  }
  async waitFor(expression, timeoutMs) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      if (await this.eval(expression)) return true;
      await sleep(50);
    }
    return false;
  }
}

// ---------------------------------------------------------------------------
// Chrome 與 ui_preview
// ---------------------------------------------------------------------------

async function startChrome(cdpPort) {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), 'cockpit-chrome-i18n-'));
  const chrome = spawn(
    CHROME,
    [
      '--headless=new',
      '--disable-gpu',
      '--no-first-run',
      '--lang=en-US',
      `--remote-debugging-port=${cdpPort}`,
      '--remote-allow-origins=*',
      `--user-data-dir=${udd}`,
      '--window-size=1400,1000',
      'about:blank',
    ],
    { stdio: 'ignore', windowsHide: true }
  );
  chrome.on('error', (e) => pre(false, `chrome spawn error：${e.message}`));
  let page = null;
  for (let i = 0; i < 100 && !page; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${cdpPort}/json/list`);
      page = (await r.json()).find((t) => t.type === 'page');
    } catch {
      // CDP endpoint 還沒起來。
    }
    if (!page) await sleep(200);
  }
  // 等不到 page target 或 WebSocket 時，先收掉自己開的 chrome 與暫存目錄再中止。
  const abort = async (msg) => {
    killTree(chrome, 'chrome（啟動失敗）');
    await sleep(500);
    fs.rmSync(udd, { recursive: true, force: true });
    throw new Error(msg);
  };
  if (!page) await abort('chrome: page target not found');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  try {
    await new Promise((res, rej) => {
      ws.onopen = res;
      ws.onerror = rej;
    });
  } catch {
    await abort('chrome: CDP WebSocket 連不上');
  }
  const cdp = new CDP(ws);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  return { chrome, udd, ws, cdp, cdpPort };
}

async function stopChrome(handle) {
  if (!handle) return;
  try {
    handle.ws.close();
  } catch {
    // 已斷線。
  }
  killTree(handle.chrome, 'chrome');
  await sleep(500);
  try {
    fs.rmSync(handle.udd, { recursive: true, force: true });
  } catch (e) {
    pre(false, `清理暫存目錄失敗：${e.message}`);
  }
}

// 起一個 ui_preview（埠由 COCKPIT_PREVIEW_LISTEN 指定）。
function startServer(port, extraEnv = {}) {
  const env = {
    ...process.env,
    COCKPIT_PREVIEW_LISTEN: `127.0.0.1:${port}`,
    COCKPIT_PREVIEW_PUSH_MS: '100',
    ...extraEnv,
  };
  const proc = spawn(UI_PREVIEW_EXE, [], { stdio: ['ignore', 'ignore', 'pipe'], windowsHide: true, env });
  let stderr = '';
  proc.stderr.setEncoding('utf8');
  proc.stderr.on('data', (c) => {
    stderr += c;
  });
  proc.on('error', (e) => pre(false, `ui_preview spawn error：${e.message}`));
  return { proc, spawnAt: Date.now(), stderr: () => stderr };
}

async function waitUp(port) {
  for (let i = 0; i < 50; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/api/state`)).ok) return true;
    } catch {
      // 還沒起來。
    }
    await sleep(200);
  }
  return false;
}

// ---------------------------------------------------------------------------
// ① 字典一致性（vm 沙箱載入 i18n.js；不需要瀏覽器）
// ---------------------------------------------------------------------------

const placeholdersOf = (s) => [...new Set([...String(s).matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort();

// 在沙箱中載入 i18n.js；回傳 { win, warns }（warns 為 console.warn 收到的訊息）。
function loadI18nInSandbox({ saved = null, languages = ['en-US'], timeZone = 'America/New_York', setItemThrows = false, readyState = 'complete' } = {}) {
  const warns = [];
  const htmlAttrs = {};
  const htmlClasses = new Set();
  const classLog = []; // 'add:xxx'／'remove:xxx'，依序
  const docListeners = {};
  const timers = [];
  const listeners = {};
  const reloads = { n: 0 };
  const writes = [];
  const sandbox = {
    console: { warn: (m) => warns.push(String(m)), log() {}, error() {} },
    navigator: { languages, language: languages[0] },
    Intl: { DateTimeFormat: () => ({ resolvedOptions: () => ({ timeZone }) }) },
    document: {
      readyState,
      documentElement: {
        setAttribute: (k, v) => { htmlAttrs[k] = v; },
        classList: {
          add: (c) => { htmlClasses.add(c); classLog.push(`add:${c}`); },
          remove: (c) => { htmlClasses.delete(c); classLog.push(`remove:${c}`); },
        },
      },
      querySelectorAll: () => [],
      addEventListener(type, fn) { (docListeners[type] = docListeners[type] || []).push(fn); },
    },
    location: { reload() { reloads.n += 1; } },
    addEventListener(type, fn) { (listeners[type] = listeners[type] || []).push(fn); },
    setTimeout(fn, ms) { timers.push({ fn, ms }); return timers.length; },
  };
  sandbox.localStorage = {
    getItem: () => saved,
    setItem(k, v) {
      if (setItemThrows) throw new Error('模擬：本機儲存不可用');
      writes.push([k, v]);
    },
    removeItem() {},
  };
  sandbox.window = sandbox;
  vm.createContext(sandbox);
  vm.runInContext(fs.readFileSync(I18N_JS, 'utf8'), sandbox, { filename: 'i18n.js' });
  return { win: sandbox, warns, htmlAttrs, htmlClasses, classLog, docListeners, timers, listeners, reloads, writes };
}

// 把原始碼逐行剝成「只剩真正的程式碼」的字串陣列（供硬編碼 CJK 與遮蔽掃描；階段審查 1 M5）：
//   - 區塊註解（可跨行）與 `//` 行尾註解整段拿掉（`//` 要在字串之外才算：字串裡的 http:// 不算註解）；
//   - `console.warn(` 的第一個引數（到同一層級的第一個逗號或右括號為止）裡的字串字面值內容清空；第二個以後的引數、
//     同一行的其他程式碼都保留，照樣被掃描；
//   - 其他字串字面值保留原樣（硬編碼的 CJK 字串要抓得到）。
// 限制：逐行掃描、字串不跨行；正規表示式字面值裡的引號可能讓同一行的字串狀態錯亂，目前這七個檔沒有這種寫法
// （掃描器本身的變異樣本在 part1 驗證）。
function stripSource(text) {
  const out = [];
  let inBlock = false;
  for (const line of text.split(/\r?\n/)) {
    let res = '';
    let i = 0;
    let quote = null;
    let warnDepth = -1; // >= 0：正在 console.warn( 的第一個引數裡，值是括號深度
    while (i < line.length) {
      const c = line[i];
      if (inBlock) {
        if (c === '*' && line[i + 1] === '/') {
          inBlock = false;
          i += 2;
        } else i += 1;
        continue;
      }
      if (quote) {
        if (c === '\\') {
          if (warnDepth < 0) res += c + (line[i + 1] || '');
          i += 2;
          continue;
        }
        if (c === quote) {
          quote = null;
          res += c;
          i += 1;
          continue;
        }
        if (warnDepth < 0) res += c;
        i += 1;
        continue;
      }
      if (c === '"' || c === "'" || c === '`') {
        quote = c;
        res += c;
        i += 1;
        continue;
      }
      if (c === '/' && line[i + 1] === '/') break;
      if (c === '/' && line[i + 1] === '*') {
        inBlock = true;
        i += 2;
        continue;
      }
      if (warnDepth < 0 && line.startsWith('console.warn(', i)) {
        res += 'console.warn(';
        i += 'console.warn('.length;
        warnDepth = 0;
        continue;
      }
      if (warnDepth >= 0) {
        if (c === '(') warnDepth += 1;
        else if (c === ')') {
          if (warnDepth === 0) warnDepth = -1;
          else warnDepth -= 1;
        } else if (c === ',' && warnDepth === 0) warnDepth = -1;
      }
      res += c;
      i += 1;
    }
    // JSDoc 風格的續行（`  * …`）整行是註解。
    out.push(/^\s*\*/.test(line) ? '' : res);
  }
  return out;
}

// 函式內遮蔽模組層級 t／tn 的樣式（階段審查 1 M2）：var／let／const 宣告、函式參數、箭頭函式參數、catch 參數。
// 模組層級的 `  var t = window.cockpitI18n.t;`（縮排 2 格、右側是 window.cockpitI18n）是唯一允許的宣告。
function findShadowing(code) {
  const found = [];
  const moduleLevel = /^ {2}var (t|tn) = window\.cockpitI18n\.(t|tn);$/.test(code);
  if (!moduleLevel && /\b(?:var|let|const)\s+(?:t|tn)\b/.test(code)) found.push('declaration');
  for (const fm of code.matchAll(/\bfunction\b[^()]*\(([^)]*)\)/g)) {
    if (fm[1].split(',').map((x) => x.trim()).some((x) => x === 't' || x === 'tn')) found.push('function parameter');
  }
  if (/\([^()]*\b(?:t|tn)\b[^()]*\)\s*=>|(?:^|[^.\w])(?:t|tn)\s*=>|\bcatch\s*\(\s*(?:t|tn)\s*\)/.test(code)) found.push('arrow／catch parameter');
  return found;
}

function part1() {
  log('=== ① 字典一致性 ===');
  let loaded;
  try {
    loaded = loadI18nInSandbox();
  } catch (e) {
    check(false, `① i18n.js 可在沙箱載入（${e.message}）`);
    return;
  }
  const api = loaded.win.cockpitI18n;
  check(api && typeof api.t === 'function' && typeof api.setLang === 'function' && typeof api.resolveLang === 'function',
    '① window.cockpitI18n 提供 t／setLang／resolveLang');
  const { zh, en } = api.dictionaries;
  const zhKeys = Object.keys(zh).sort();
  const enKeys = Object.keys(en).sort();
  check(zhKeys.length > 0, `① 繁中字典不是空的（${zhKeys.length} 個鍵）`);
  const onlyZh = zhKeys.filter((k) => !(k in en));
  const onlyEn = enKeys.filter((k) => !(k in zh));
  check(onlyZh.length === 0 && onlyEn.length === 0, `① 兩份字典鍵集合相同（只在繁中：${J(onlyZh)}；只在英文：${J(onlyEn)}）`);
  const badPh = zhKeys.filter((k) => k in en && !sameSet(placeholdersOf(zh[k]), placeholdersOf(en[k])));
  check(badPh.length === 0, `① 每個鍵兩種語言的具名佔位符集合相同（不一致：${J(badPh.map((k) => [k, placeholdersOf(zh[k]), placeholdersOf(en[k])]))}）`);
  const emptyVals = [...zhKeys.filter((k) => !String(zh[k]).trim()), ...enKeys.filter((k) => !String(en[k]).trim())];
  check(emptyVals.length === 0, `① 沒有空字串的字典值（${J(emptyVals)}）`);
  const badKeyName = zhKeys.filter((k) => !(/^[a-z]+(\.[a-zA-Z][a-zA-Z0-9]*)+$/.test(k) || /^msg\.[a-z][a-z0-9_]*$/.test(k))); // msg.<snake_case 代碼>（design D4）
  check(badKeyName.length === 0, `① 鍵名符合「<模組>.<區塊>.<名稱>」命名慣例，後端訊息為 msg.<代碼>（不符：${J(badKeyName)}）`);

  // t() 行為（沙箱語言為 en；另載一份 zh 驗繁中）。
  const enT = api.t;
  check(enT('index.tab.files') === en['index.tab.files'], '① t(key) 回傳目前語言字典值');
  check(enT('index.channel.title', { state: 'connected' }) === en['index.channel.title'].replace('{state}', 'connected'),
    '① t(key, params) 以 {name} 代入參數');
  check(enT('index.channel.title') === en['index.channel.title'], '① 沒給的佔位符原樣保留');
  const missing = enT('no.such.key');
  enT('no.such.key');
  check(missing === 'no.such.key', '① 缺鍵時回傳鍵本身');
  check(loaded.warns.filter((w) => w.includes('no.such.key')).length === 1, `① 缺鍵只 console.warn 一次（實際 ${loaded.warns.length} 次）`);
  const zhLoaded = loadI18nInSandbox({ saved: 'zh' });
  check(zhLoaded.win.cockpitI18n.lang === 'zh' && zhLoaded.win.cockpitI18n.t('index.tab.files') === zh['index.tab.files'],
    '① saved=zh 時 t() 回傳繁中字典值');
  check(zhLoaded.htmlAttrs.lang === 'zh-Hant' && loaded.htmlAttrs.lang === 'en', `① <html lang> 設為 zh-Hant／en（${J([zhLoaded.htmlAttrs.lang, loaded.htmlAttrs.lang])}）`);

  // 第一次繪製前決定語言（階段審查 1 M1；spec「任何介面文字繪製之前決定語言」）：i18n.js 在 <head> 執行（readyState=loading）時，
  // <html lang> 立刻設好；英文先加 i18n-pending（藏起帶 data-i18n 的靜態節點），DOMContentLoaded 套用翻譯後移除，另有 1.5 秒保險計時器；
  // 繁中、或腳本在文件解析完才執行（readyState 不是 loading）時不加。
  {
    const enLoading = loadI18nInSandbox({ saved: 'en', readyState: 'loading' });
    check(enLoading.htmlAttrs.lang === 'en' && enLoading.htmlClasses.has('i18n-pending') && enLoading.classLog.join() === 'add:i18n-pending',
      `① readyState=loading＋英文：載入當下就設好 <html lang> 並加 i18n-pending（${J([enLoading.htmlAttrs.lang, enLoading.classLog])}）`);
    check((enLoading.docListeners.DOMContentLoaded || []).length === 1 && enLoading.timers.length === 1 && enLoading.timers[0].ms === 1500,
      `① readyState=loading＋英文：等 DOMContentLoaded 套用，並註冊 1.5 秒保險計時器（${J([(enLoading.docListeners.DOMContentLoaded || []).length, enLoading.timers.map((t) => t.ms)])}）`);
    enLoading.docListeners.DOMContentLoaded.forEach((f) => f());
    check(!enLoading.htmlClasses.has('i18n-pending'), '① DOMContentLoaded 套用後移除 i18n-pending');
    const enTimer = loadI18nInSandbox({ saved: 'en', readyState: 'loading' });
    enTimer.timers[0].fn();
    check(!enTimer.htmlClasses.has('i18n-pending'), '① 保險計時器觸發時（DOMContentLoaded 遲遲不來）也移除 i18n-pending');
    const zhLoading = loadI18nInSandbox({ saved: 'zh', readyState: 'loading' });
    check(zhLoading.htmlAttrs.lang === 'zh-Hant' && zhLoading.classLog.length === 0 && zhLoading.timers.length === 0,
      `① readyState=loading＋繁中：<html lang> 設好、不加 i18n-pending、不起計時器（${J([zhLoading.htmlAttrs.lang, zhLoading.classLog, zhLoading.timers.length])}）`);
    const enDone = loadI18nInSandbox({ saved: 'en', readyState: 'complete' });
    check(enDone.classLog.length === 0 && enDone.htmlAttrs.lang === 'en', '① readyState=complete＋英文：直接套用、不加 i18n-pending');
  }
  {
    const html0 = fs.readFileSync(INDEX_HTML, 'utf8');
    const headEnd = html0.indexOf('</head>');
    const i18nAt = html0.indexOf('<script src="/app/i18n.js"></script>');
    check(headEnd > 0 && i18nAt > 0 && i18nAt < headEnd && html0.indexOf('<script src=') === i18nAt,
      `① index.html：i18n.js 在 <head> 內且是第一個 <script src>（位置 ${i18nAt}，</head> 在 ${headEnd}）`);
    const css = fs.readFileSync(path.join(REPO, 'cockpit', 'assets', 'app', 'style.css'), 'utf8');
    check(/html\.i18n-pending\s+\[data-i18n\]\s*\{\s*visibility:\s*hidden;?\s*\}/.test(css), '① style.css 有 html.i18n-pending [data-i18n] { visibility: hidden } 規則');
  }

  // 切換按鈕用的字典鍵、canPersist、setLang 寫入失敗、storage 事件（task 1.2）。
  const LANG_KEYS = ['render.lang.text', 'render.lang.code', 'render.lang.aria', 'render.lang.needsStorage'];
  check(LANG_KEYS.every((k) => k in zh && k in en), `① 字典有切換按鈕的 ${LANG_KEYS.length} 個鍵`);
  check(zh['render.lang.text'] === 'EN' && zh['render.lang.code'] === 'en' && zh['render.lang.aria'] === 'Switch to English'
    && en['render.lang.text'] === '中文' && en['render.lang.code'] === 'zh-Hant' && en['render.lang.aria'] === '切換為繁體中文',
  '① 切換按鈕文字／lang／可及名稱與 spec 逐字相同（繁中介面 EN／en／Switch to English；英文介面 中文／zh-Hant／切換為繁體中文）');
  check(zh['render.lang.needsStorage'] === '需要瀏覽器儲存空間才能切換語言'
    && en['render.lang.needsStorage'] === 'Switching language needs browser storage', '① 儲存不可用時的 title 文字與裁決逐字相同');
  check(api.canPersist === true, '① localStorage 可寫時 canPersist 為 true');
  const throwing = loadI18nInSandbox({ setItemThrows: true });
  check(throwing.win.cockpitI18n.canPersist === false, '① setItem 丟例外時 canPersist 為 false');
  throwing.win.cockpitI18n.setLang('zh');
  check(throwing.reloads.n === 0, `① setLang 寫入失敗時不重新載入（reload ${throwing.reloads.n} 次）`);
  const okSet = loadI18nInSandbox();
  okSet.win.cockpitI18n.setLang('zh');
  check(okSet.reloads.n === 1 && okSet.writes.some((w) => w[0] === 'cockpit.lang' && w[1] === 'zh'), '① setLang 寫入成功時寫 cockpit.lang 並重新載入一次');
  const fire = (ld, ev) => (ld.listeners.storage || []).forEach((f) => f(ev));
  check((okSet.listeners.storage || []).length === 1, '① 載入時註冊一個 storage 事件監聽');
  const sEn = loadI18nInSandbox({ saved: 'en' }); // 目前英文
  fire(sEn, { key: 'cockpit.lang', newValue: 'zh' });
  check(sEn.reloads.n === 1, '① storage：cockpit.lang 變成另一種語言 → 重新載入');
  const sEn2 = loadI18nInSandbox({ saved: 'en' });
  fire(sEn2, { key: 'cockpit.lang', newValue: 'en' });
  fire(sEn2, { key: 'cockpit.other', newValue: 'zh' });
  check(sEn2.reloads.n === 0, '① storage：同語言、或別的鍵 → 不重新載入');

  // tn()：依數量選 .one／.other（task 2.1 裁決）。
  check(typeof api.tn === 'function', '① window.cockpitI18n 提供 tn');
  const enTn = api.tn;
  const zhTn = zhLoaded.win.cockpitI18n.tn;
  check(enTn('render.warning.count', 1) === '1 warning' && enTn('render.warning.count', 3) === '3 warnings' && enTn('render.warning.count', 0) === '0 warnings',
    `① tn：n=1 取 .one、其餘（含 0）取 .other，{n} 自動代入（${J([enTn('render.warning.count', 1), enTn('render.warning.count', 3), enTn('render.warning.count', 0)])}）`);
  check(zhTn('render.warning.count', 1) === '警告 1' && zhTn('render.warning.count', 3) === '警告 3' && zhTn('render.binding.ambiguous', 2) === '歧義（2）',
    '① tn（繁中）：警告 N、歧義（N）與改動前逐字相同');
  check(enTn('render.binding.ambiguous', 2, { n: 99 }) === 'Ambiguous (2)', '① tn：n 蓋過 params 裡同名的 n');
  check(enTn('notify.summary.title', 4) === 'Cockpit: 4 items need attention' && enTn('notify.summary.title', 1) === 'Cockpit: 1 item needs attention'
    && zhTn('notify.summary.title', 4) === 'Cockpit：4 件事需要注意' && zhTn('notify.summary.title', 1) === 'Cockpit：1 件事需要注意',
  `① tn（notify）：合併通知標題單複數與改動前逐字相同（${J([enTn('notify.summary.title', 4), enTn('notify.summary.title', 1), zhTn('notify.summary.title', 4)])}）`);
  check(zh['notify.body.namesSeparator'] === '、' && en['notify.body.namesSeparator'] === ', ',
    `① 綁定名稱的分隔符依語言（繁中「、」、英文「, 」；實際 ${J([zh['notify.body.namesSeparator'], en['notify.body.namesSeparator']])}）`);
  const doneKeys = zhKeys.filter((k) => k.split('.').some((seg) => seg.startsWith('done') || seg.startsWith('agentDone')));
  check(['notify.kind.done.desc', 'notify.title.done'].every((k) => doneKeys.includes(k)),
    `① 描述 HERDR done 的鍵都在 design D5 的守門命名下（守門鍵 ${J(doneKeys)}；禁字由 cockpit/tests/http.rs 檢查）`);
  const pluralBases = [...new Set(zhKeys.filter((k) => /\.(one|other)$/.test(k)).map((k) => k.replace(/\.(one|other)$/, '')))];
  const badPlural = pluralBases.filter((b) => !(`${b}.one` in zh && `${b}.other` in zh && `${b}.one` in en && `${b}.other` in en));
  check(pluralBases.length > 0 && badPlural.length === 0, `① 每組複數鍵兩種語言都有 .one 與 .other（${pluralBases.length} 組；缺：${J(badPlural)}）`);
  const zhPluralDiffer = pluralBases.filter((b) => zh[`${b}.one`] !== zh[`${b}.other`]);
  check(zhPluralDiffer.length === 0, `① 繁中的 .one 與 .other 同文（${J(zhPluralDiffer)}）`);

  // 程式碼裡的字典鍵（t("…")／tn("…")、條件式裡的字串）都存在，字典裡 render／actions／output 的鍵也都有人用
  // （缺鍵執行期只會 console.warn、畫面顯示鍵本身；多餘的鍵是死字串）。
  const APP_DIR = path.join(REPO, 'cockpit', 'assets', 'app');
  const SRC_FILES = ['render.js', 'actions.js', 'output.js', 'files.js', 'viewers.js', 'git.js', 'notify.js'];
  const srcs = SRC_FILES.map((f) => [f, fs.readFileSync(path.join(APP_DIR, f), 'utf8')]);
  const usedKeys = new Set();
  for (const [, text] of srcs) {
    for (const m of text.matchAll(/"((?:render|actions|output|files|viewers|git|notify|index)\.[A-Za-z0-9.]+)"/g)) usedKeys.add(m[1]);
  }
  const missingInDict = [...usedKeys].filter((k) => !(k in zh) && !(`${k}.one` in zh && `${k}.other` in zh));
  check(usedKeys.size > 30 && missingInDict.length === 0, `① render／actions／output／files／viewers／git／notify 用到的字典鍵（${usedKeys.size} 個）都在字典裡（缺：${J(missingInDict)}）`);
  const allSrc = fs.readdirSync(APP_DIR).filter((f) => f.endsWith('.js') && f !== 'i18n.js').map((f) => fs.readFileSync(path.join(APP_DIR, f), 'utf8')).join('\n') + fs.readFileSync(INDEX_HTML, 'utf8');
  const deadKeys = zhKeys.filter((k) => /^(render|actions|output|files|viewers|git|notify)\./.test(k)).filter((k) => !allSrc.includes(`"${k.replace(/\.(one|other)$/, '')}"`));
  check(deadKeys.length === 0, `① render／actions／output／files／viewers／git／notify 的字典鍵都有程式碼用到（沒人用：${J(deadKeys)}）`);
  // 這七個檔不再有硬編碼的繁中字串（註解與開發者看的 console.warn 訊息除外）。console.warn 只略過「它的第一個引數」裡的字串字面值，
  // 同一行其他程式碼照樣檢查；`//` 只在字串之外才算註解（字串裡的 http:// 不會把後面的程式碼吃掉）；CJK 標點（「」、。（）：等）也算。
  const hardCoded = [];
  const shadows = [];
  const pluralMisuse = [];
  for (const [f, text] of srcs) {
    stripSource(text).forEach((code, i) => {
      if (CJK_RE.test(code)) hardCoded.push(`${f}:${i + 1}`);
      if (findShadowing(code).length) shadows.push(`${f}:${i + 1}`);
      for (const m of code.matchAll(/(?<![\w.])t\(\s*"([A-Za-z0-9.]+)"/g)) if (pluralBases.includes(m[1])) pluralMisuse.push(`${f}:${i + 1}:${m[1]}`);
    });
  }
  check(hardCoded.length === 0, `① render／actions／output／files／viewers／git／notify 的程式碼（註解與 console.warn 的訊息字串除外）沒有硬編碼的 CJK 字串或標點（${J(hardCoded.slice(0, 6))}）`);
  // 沒有函式內的區域變數／參數遮蔽模組層級的 t() 與 tn()（階段審查 1 M2）：var 會提升到整個函式，日後在該函式任何位置加 t("…") 就丟 TypeError。
  check(shadows.length === 0, `① 沒有函式內的變數或參數遮蔽模組層級的 t／tn（${J(shadows)}）`);
  // t() 不得用複數基底鍵（那是 tn() 的事；用 t("render.warning.count") 會顯示鍵名；階段審查 1 M5）。
  check(pluralBases.length > 0 && pluralMisuse.length === 0, `① t() 沒有用到複數基底鍵（${pluralBases.length} 組；該用 tn()，違規 ${J(pluralMisuse)}）`);
  // 掃描器本身要有牙齒（變異樣本）：該抓的抓得到、該略過的略過。
  {
    const scan = (src) => stripSource(src).some((l) => CJK_RE.test(l));
    check(scan('el("a", "x", "「" + name + "」");') && scan('var s = "（備註）";') && scan('x("，");') && scan('x("、。");'), '① 硬編碼掃描：抓得到 CJK 標點「」（）， 與 、。');
    check(scan('console.warn("url http://x", "中文");') && scan('console.warn("x"); el("a", "x", "中文");') && scan('el("a", "http://x" + "中文");'),
      '① 硬編碼掃描：console.warn 之後同一行的程式碼、字串裡有 http:// 的行，後面的 CJK 仍然抓得到');
    check(!scan('console.warn("分頁還原：略過", e);') && !scan("console.warn('分頁還原：「' + kind + '」略過', entry);") && !scan('var a = 1; // 註解：中文')
      && !scan('/* 區塊註解：中文 */ var a = 1;') && !scan('  * JSDoc 中文'),
    '① 硬編碼掃描：console.warn 第一個引數的字串、行尾註解、區塊註解都略過');
    check(scan('console.warn("x", "中文");') && scan('console.warn(f("a"), "中文");'), '① 硬編碼掃描：console.warn 的第二個以後引數不略過');
    const sh = (src) => stripSource(src).some((l) => findShadowing(l).length > 0);
    check(sh('    for (var t = 0; t < n; t += 1) {') && sh('  function f(t) {') && sh('x.forEach(function (a, t) {') && sh('    var t = at.indexOf("T");')
      && sh('a.map((t) => t.id);') && sh('a.map(t => t.id);') && sh('a.map((x, tn) => x);') && sh('  var tn = 3;') && sh('} catch (t) {'),
    '① 遮蔽掃描：抓得到迴圈變數 t、函式參數 t／tn、區域 var t／tn、箭頭函式與 catch 參數');
    check(!sh('  var t = window.cockpitI18n.t;') && !sh('  var tn = window.cockpitI18n.tn;') && !sh('    var tab = tabs[t];') && !sh('    x(t("a.b"));') && !sh('    var tIdx = 1; var ti = 2;'),
      '① 遮蔽掃描：模組層級的 var t／tn、拿 t() 當函式呼叫、名字只是含 t 的變數都不誤報');
    check(/(?<![\w.])t\(\s*"([A-Za-z0-9.]+)"/.exec('x(t("render.warning.count", 3))')[1] === 'render.warning.count' && pluralBases.includes('render.warning.count'),
      '① 複數基底鍵檢查的樣式抓得到 t("render.warning.count")');
  }

  // 後端訊息代碼（task 3.3；design D4）：3.1 的錯誤本體代碼與 3.2 的投影代碼，每一個都有 msg.* 鍵（兩種語言、佔位符一致已由上面檢查）；
  // 下表逐一斷言兩份範本代入參數後的字串：繁中與後端原文逐字相同（原文見 cockpit-core/src/message.rs、rejection.rs、
  // progress_service.rs、http.rs 的 Display），英文是譯文。新增代碼時這張表與字典要一起補。
  const MSG_CASES = [
    ['invalid_op', { op: 'frobnicate' }, '不是合法的操作：frobnicate', 'Not a valid operation: frobnicate'],
    ['unknown_project', { id: 'p9' }, 'project 不存在：p9', 'Unknown project: p9'],
    ['unknown_task', { id: 't9' }, 'task 不存在：t9', 'Unknown task: t9'],
    ['unknown_workstream', { id: 'w9' }, 'workstream 不存在：w9', 'Unknown workstream: w9'],
    ['already_last_stage', {}, '已是最後一個 Stage', 'Already at the last stage'],
    ['already_first_stage', {}, '已是第一個 Stage', 'Already at the first stage'],
    ['already_marked', {}, '已有標記', 'Already marked'],
    ['task_not_in_workstream', {}, 'task 不屬於該 workstream', 'The task does not belong to this workstream'],
    ['runtime_not_registered', {}, 'runtime 未登記', 'Runtime is not registered'],
    ['runtime_not_connected', {}, 'runtime 未連線', 'Runtime is not connected'],
    ['pane_not_found', {}, 'pane 不存在', 'Pane does not exist'],
    ['pane_exited', {}, 'pane 已 exited', 'Pane has exited'],
    ['persist_failed', { detail: 'disk full' }, '寫入狀態檔失敗：disk full', 'Failed to write the state file: disk full'],
    ['internal_error', {}, '寫入任務異常結束', 'The write task ended abnormally'],
    // pane_not_bound、forbidden_source、method_not_allowed 的後端原文有多種（有無 task／pane 參數、哪個檢查失敗、哪個端點），
    // 代碼只有一個，範本不依賴參數、是概括句，所以繁中範本不逐字等於其中任何一種原文（繁中介面顯示原文，不受影響）。
    ['pane_not_bound', {}, 'pane 未綁定到該 task 所屬的 workstream', "The pane is not bound to this task's workstream"],
    ['pane_not_bound', { task: 'be-1', pane: 'w1:p2' }, 'pane 未綁定到該 task 所屬的 workstream', "The pane is not bound to this task's workstream"],
    ['missing_pane_id', {}, '缺少 X-Herdr-Pane-Id 標頭（值為 pane 內的 HERDR_PANE_ID）', 'Missing X-Herdr-Pane-Id header (the value is HERDR_PANE_ID inside the pane)'],
    ['forbidden_source', {}, '請求來源不被接受，請從本機的 Cockpit 頁面開啟', 'Request origin not accepted. Open this from the local Cockpit page'],
    ['method_not_allowed', {}, '這個操作不被接受', 'This operation is not allowed'],
    ['runtime_not_found', { runtime: 'nope' }, 'runtime 不存在：nope', 'Unknown runtime: nope'],
    ['pane_gone', { pane: 'w1:p9' }, 'pane 不存在：w1:p9', 'Pane does not exist: w1:p9'],
    ['output_read_failed', { detail: 'socket closed' }, '讀取 pane 輸出失敗：socket closed', 'Failed to read pane output: socket closed'],
    ['read_timeout', {}, '讀取逾時', 'Read timed out'],
    ['wsl_distro_not_running', { distro: 'Ubuntu-24.04' }, 'WSL 發行版 Ubuntu-24.04 未啟動', 'WSL distro Ubuntu-24.04 is not running'],
    ['wsl_probe_failed', { detail: 'exit 1' }, 'WSL 探測失敗：exit 1', 'WSL probe failed: exit 1'],
    ['snapshot_failed', { detail: 'broken pipe' }, 'snapshot 失敗：broken pipe', 'Snapshot failed: broken pipe'],
    ['seed_snapshot_failed', { detail: 'broken pipe' }, 'seed snapshot 失敗：broken pipe', 'Seed snapshot failed: broken pipe'],
    ['lifecycle_subscribe_failed', { detail: 'refused' }, 'L 訂閱建立失敗：refused', 'Failed to open the L subscription: refused'],
    ['status_subscribe_failed', { detail: 'refused' }, 'S 訂閱建立失敗：refused', 'Failed to open the S subscription: refused'],
    ['status_resubscribe_failed', { detail: 'refused' }, 'S 重開失敗：refused', 'Failed to reopen the S subscription: refused'],
    ['protocol_untested', { protocol: '23', tested: '20..=22' }, 'HERDR protocol 23 不在已測範圍 20..=22', 'HERDR protocol 23 is outside the tested range 20..=22'],
    ['event_stream_ended', {}, '事件流結束', 'Event stream ended'],
    ['event_connection_error', { label: 'L', detail: 'reset' }, 'L 連線錯誤：reset', 'L connection error: reset'],
    ['event_connection_ended', { label: 'S' }, 'S 連線結束', 'S connection ended'],
    ['task_stage_reset', { task: 'docs-1', stage: 'Draft', start: 'Spec' },
      'task docs-1 的 stage「Draft」已不在 pipeline 的 stages 中，已退回起始 stage「Spec」',
      'Task docs-1: stage "Draft" is no longer in the pipeline stages; reset to the start stage "Spec"'],
    ['drift_workspace_not_found', { id: 'wZ' }, 'workspace wZ 不存在', 'Workspace wZ does not exist'],
    ['drift_tab_not_found', { id: 'wZ:t9' }, 'tab wZ:t9 不存在', 'Tab wZ:t9 does not exist'],
    ['drift_pane_not_found', { id: 'wZ:p9' }, 'pane wZ:p9 不存在', 'Pane wZ:p9 does not exist'],
    ['drift_runtime_not_registered', { id: 'ghost' }, 'runtime ghost 未登記', 'Runtime ghost is not registered'],
    ['event_payload_unparsable', { event: 'pane_created', detail: 'missing field' }, 'pane_created payload 無法解析：missing field',
      'Could not parse the pane_created event payload: missing field'],
    ['raw', { text: 'connection refused (os error 111)' }, 'connection refused (os error 111)', 'connection refused (os error 111)'],
  ];
  const msgKeys = zhKeys.filter((k) => k.startsWith('msg.')).sort();
  const wantMsgKeys = [...new Set(MSG_CASES.map(([c]) => `msg.${c}`))].sort();
  check(sameSet(msgKeys, wantMsgKeys) && sameSet(enKeys.filter((k) => k.startsWith('msg.')).sort(), wantMsgKeys),
    `① 字典的 msg.* 鍵與 3.1／3.2 的代碼清單一致（${wantMsgKeys.length} 個；多：${J(msgKeys.filter((k) => !wantMsgKeys.includes(k)))}；缺：${J(wantMsgKeys.filter((k) => !msgKeys.includes(k)))}）`);
  const msgWrong = [];
  for (const [code, params, zhText, enText] of MSG_CASES) {
    const z = zhLoaded.win.cockpitI18n.t(`msg.${code}`, params);
    const e = enT(`msg.${code}`, params);
    if (z !== zhText || e !== enText) msgWrong.push([code, z, e]);
  }
  const enTemplateCjk = enKeys.filter((k) => k.startsWith('msg.') && CJK_RE.test(en[k]));
  check(enTemplateCjk.length === 0, `① 英文的 msg.* 範本不含 CJK 字元（${J(enTemplateCjk)}）`);
  // 沒有參數也能顯示的代碼：範本不含佔位符，英文介面遇到缺 params 的本體（例如寫入鎖內重驗的 pane_not_bound）仍顯示英文。
  const noParamCodes = ['pane_not_bound', 'missing_pane_id', 'forbidden_source', 'method_not_allowed', 'already_last_stage', 'read_timeout', 'internal_error'];
  check(noParamCodes.every((c) => placeholdersOf(en[`msg.${c}`]).length === 0 && api.tMsg({ code: c }, '繁中原文') === en[`msg.${c}`]),
    '① 沒有參數的代碼（pane_not_bound／missing_pane_id／forbidden_source／method_not_allowed 等）缺 params 時英文介面仍顯示英文範本');
  check(msgWrong.length === 0, `① 每個代碼的繁中範本等於後端原文、英文範本等於預期譯文（不符：${J(msgWrong.slice(0, 3))}）`);
  const enMsg = api.tMsg;
  const zhMsg = zhLoaded.win.cockpitI18n.tMsg;
  const warnsBefore = loaded.warns.length;
  check(typeof enMsg === 'function' && typeof zhMsg === 'function', '① window.cockpitI18n 提供 tMsg');
  check(enMsg({ code: 'wsl_distro_not_running', params: { distro: 'Ubuntu-24.04' } }, 'WSL 發行版 Ubuntu-24.04 未啟動') === 'WSL distro Ubuntu-24.04 is not running',
    '① tMsg（英文）：字典有代碼時用範本加參數，不用原文');
  check(enMsg({ code: 'unknown_project', params: { id: '專案甲' } }, 'x') === 'Unknown project: 專案甲', '① tMsg：參數值原樣代入、不翻譯');
  check(enMsg({ code: 'persist_failed', params: { detail: '{id} {x}' } }, 'x') === 'Failed to write the state file: {id} {x}', '① tMsg：參數值裡的 {…} 原樣保留（不二次代入）');
  check(enMsg({ code: 'raw', params: { text: 'HERDR 原文' } }, 'zzz') === 'HERDR 原文', '① tMsg：raw 代碼原樣顯示 text');
  check(enMsg({ code: 'no_such_code', params: { a: '1' } }, '後端原文') === '後端原文', '① tMsg：字典沒有的代碼退回原文');
  check(!enMsg({ code: 'no_such_code' }, '後端原文').includes('no_such_code'), '① tMsg：不顯示代碼本身');
  check(enMsg(null, 'r') === 'r' && enMsg(undefined, 'r') === 'r' && enMsg({}, 'r') === 'r' && enMsg({ code: 5 }, 'r') === 'r' && enMsg({ code: '' }, 'r') === 'r',
    '① tMsg：沒有 msgObj、缺 code、code 不是字串或是空字串都退回原文');
  check(enMsg({ code: '__proto__' }, 'r') === 'r' && enMsg({ code: 'constructor' }, 'r') === 'r' && enMsg(JSON.parse('{"code":"invalid_op","params":{"__proto__":{"op":"x"}}}'), 'r') === 'r',
    '① tMsg：原型鏈上的名稱不當代碼或參數');
  check(enMsg({ code: 'task_stage_reset', params: { task: 'a' } }, '原文') === '原文' && enMsg({ code: 'invalid_op' }, '原文') === '原文',
    '① tMsg：範本的佔位符缺參數時退回原文（不顯示 {op} 這種殘骸）');
  check(enMsg({ code: 'no_such_code' }) === '' && enMsg(null, undefined) === '', '① tMsg：連原文都沒有時回傳空字串');
  check(enMsg({ code: 'already_last_stage' }, '') === 'Already at the last stage', '① tMsg：原文是空字串時仍用範本');
  check(loaded.warns.length === warnsBefore, `① tMsg 不 console.warn（未知代碼、缺欄位都靜默；新增 ${loaded.warns.length - warnsBefore} 則）`);
  check(zhMsg({ code: 'already_last_stage' }, '已是最後一個 Stage') === '已是最後一個 Stage'
    && zhMsg({ code: 'persist_failed', params: { detail: 'x' } }, '寫入狀態檔失敗（C:\\s.json）：x') === '寫入狀態檔失敗（C:\\s.json）：x',
  '① tMsg（繁中）：有原文就顯示原文（範本放不下的細節，例如狀態檔路徑，不遺失）');
  check(zhMsg({ code: 'wsl_distro_not_running', params: { distro: 'U' } }, '') === 'WSL 發行版 U 未啟動' && zhMsg({ code: 'zzz' }, '原文') === '原文' && zhMsg({ code: 'zzz' }) === '',
    '① tMsg（繁中）：原文缺漏時用繁中範本；沒有範本也沒有原文回傳空字串');

  // index.html 的每個 data-i18n 鍵都在字典裡，且節點內的後備文字與繁中字典值逐字相同（含 data-i18n-params 代入）。
  const html = fs.readFileSync(INDEX_HTML, 'utf8');
  const tags = [...html.matchAll(/<(\w+)([^>]*\sdata-i18n="([^"]+)"[^>]*)>([^<]*)/g)];
  check(tags.length > 0, `① index.html 有 data-i18n 節點（${tags.length} 個）`);
  const missingKeys = tags.filter((m) => !(m[3] in zh)).map((m) => m[3]);
  check(missingKeys.length === 0, `① index.html 的 data-i18n 鍵都在字典裡（缺：${J(missingKeys)}）`);
  const drift = [];
  for (const m of tags) {
    if (!(m[3] in zh)) continue;
    const attrs = m[2];
    const attrName = (attrs.match(/data-i18n-attr="([^"]+)"/) || [])[1];
    const params = (attrs.match(/data-i18n-params='([^']+)'/) || [])[1];
    const expected = zhLoaded.win.cockpitI18n.t(m[3], params ? JSON.parse(params) : null);
    const fallback = attrName ? (attrs.match(new RegExp(`\\s${attrName}="([^"]*)"`)) || [])[1] : m[4].trim();
    if (fallback !== expected) drift.push([m[3], fallback, expected]);
  }
  check(drift.length === 0, `① index.html 節點內的後備文字與繁中字典值逐字相同（不一致：${J(drift)}）`);
}

// ---------------------------------------------------------------------------
// ③ 語言決定
// ---------------------------------------------------------------------------

// resolveLang 純函式案例：[說明, 輸入, 預期]。
const NY = 'America/New_York';
const RESOLVE_CASES = [
  ['zh-SG（時區紐約）→ en', { languages: ['zh-SG'], timeZone: NY }, 'en'],
  ['zh-Hant-SG（時區紐約）→ en', { languages: ['zh-Hant-SG'], timeZone: NY }, 'en'],
  ['zh-hant（小寫，時區紐約）→ zh', { languages: ['zh-hant'], timeZone: NY }, 'zh'],
  ['zh-CN → zh', { languages: ['zh-CN'], timeZone: NY }, 'zh'],
  ['zh → zh', { languages: ['zh'], timeZone: NY }, 'zh'],
  ['zh-TW → zh', { languages: ['zh-TW'], timeZone: NY }, 'zh'],
  ['ZH-tw（大寫不分）→ zh', { languages: ['ZH-tw'], timeZone: NY }, 'zh'],
  ['zh-Hans-CN → zh', { languages: ['zh-Hans-CN'], timeZone: NY }, 'zh'],
  ['zh-HK → zh', { languages: ['zh-HK'], timeZone: NY }, 'zh'],
  ['zh-MO → zh', { languages: ['zh-MO'], timeZone: NY }, 'zh'],
  ['zh-SG 但時區 Asia/Taipei → zh（落到時區判斷）', { languages: ['zh-SG'], timeZone: 'Asia/Taipei' }, 'zh'],
  ['en-US＋Asia/Hong_Kong → zh', { languages: ['en-US'], timeZone: 'Asia/Hong_Kong' }, 'zh'],
  ['ja-JP＋Asia/Tokyo → en', { languages: ['ja-JP'], timeZone: 'Asia/Tokyo' }, 'en'],
  ['en-US＋Asia/Taipei → zh', { languages: ['en-US'], timeZone: 'Asia/Taipei' }, 'zh'],
  ['en-US、zh-TW（只看第一順位）＋紐約 → en', { languages: ['en-US', 'zh-TW'], timeZone: NY }, 'en'],
  ['saved=en 蓋過 Asia/Taipei → en', { saved: 'en', languages: ['zh-TW'], timeZone: 'Asia/Taipei' }, 'en'],
  ['saved=zh 蓋過 ja-JP＋Tokyo → zh', { saved: 'zh', languages: ['ja-JP'], timeZone: 'Asia/Tokyo' }, 'zh'],
  ['saved=fr（非法）被忽略：en-US＋紐約 → en', { saved: 'fr', languages: ['en-US'], timeZone: NY }, 'en'],
  ['saved=fr（非法）被忽略：zh-TW → zh', { saved: 'fr', languages: ['zh-TW'], timeZone: NY }, 'zh'],
  ['saved=ZH（大小寫不符）被忽略：en-US＋紐約 → en', { saved: 'ZH', languages: ['en-US'], timeZone: NY }, 'en'],
  ['沒有語言資訊（空陣列）＋Asia/Taipei → zh', { languages: [], timeZone: 'Asia/Taipei' }, 'zh'],
  ['沒有語言資訊（欄位不存在）＋紐約 → en', { timeZone: NY }, 'en'],
  ['沒有時區資訊（空字串）＋en-US → en', { languages: ['en-US'], timeZone: '' }, 'en'],
  ['沒有任何輸入 → en', {}, 'en'],
  ['zhx（非完整比對）→ en', { languages: ['zhx'], timeZone: NY }, 'en'],
  ...['Asia/Macau', 'Asia/Macao', 'Asia/Shanghai', 'Asia/Chongqing', 'Asia/Chungking', 'Asia/Harbin', 'Asia/Urumqi',
    'Asia/Kashgar', 'PRC', 'ROC', 'Hongkong'].map((tz) => [`en-US＋${tz} → zh`, { languages: ['en-US'], timeZone: tz }, 'zh']),
  ['en-US＋Asia/Tokyo → en（時區名單之外）', { languages: ['en-US'], timeZone: 'Asia/Tokyo' }, 'en'],
];

// 端到端情境：真的設定時區與 Accept-Language（navigator.languages），清 localStorage 或預先寫入，載入後讀結果。
// 靜態節點用「左欄『變更』分頁按鈕」判斷：render.js／files.js 都不會改寫它，所以載入後文字就是字典值。
const E2E_CASES = [
  { name: '台灣使用者預設繁中', acceptLanguage: 'en-US', tz: 'Asia/Taipei', saved: null, lang: 'zh', htmlLang: 'zh-Hant' },
  { name: '其他地區預設英文', acceptLanguage: 'ja-JP', tz: 'Asia/Tokyo', saved: null, lang: 'en', htmlLang: 'en' },
  { name: '只看第一順位語言（en-US、zh-TW＋紐約）', acceptLanguage: 'en-US,zh-TW', tz: NY, saved: null, lang: 'en', htmlLang: 'en' },
  { name: '手動選擇優先（cockpit.lang=en＋Asia/Taipei）', acceptLanguage: 'zh-TW', tz: 'Asia/Taipei', saved: 'en', lang: 'en', htmlLang: 'en' },
  { name: '第一順位 zh-TW 即使時區在紐約 → 繁中', acceptLanguage: 'zh-TW', tz: NY, saved: null, lang: 'zh', htmlLang: 'zh-Hant' },
  { name: '手動選擇優先（cockpit.lang=zh＋ja-JP＋Tokyo）', acceptLanguage: 'ja-JP', tz: 'Asia/Tokyo', saved: 'zh', lang: 'zh', htmlLang: 'zh-Hant' },
];
const TAB_TEXT = { zh: '變更', en: 'Changes' };
const LEFT_ARIA = { zh: '左欄', en: 'Left pane' };

// 載入頁面並等 i18n.js 與第一份投影的靜態骨架就緒。
async function loadPage(cdp, url) {
  await cdp.eval('window.__oldDoc = true');
  await cdp.send('Page.navigate', { url });
  return cdp.waitFor("!window.__oldDoc && document.readyState === 'complete' && !!window.cockpitI18n", 15000);
}

// 在同源的靜態端點（沒有 i18n.js）上設定或清除 localStorage，之後再載入儀表板。
async function setSaved(cdp, base, saved) {
  await cdp.send('Page.navigate', { url: `${base}/manifest.webmanifest` });
  await cdp.waitFor("location.pathname === '/manifest.webmanifest' && document.readyState === 'complete'", 10000);
  await cdp.fn((v) => {
    if (v === null) localStorage.removeItem('cockpit.lang');
    else localStorage.setItem('cockpit.lang', v);
    return true;
  }, saved);
}

async function setEnv(cdp, { acceptLanguage, tz }) {
  await cdp.send('Network.enable');
  await cdp.send('Emulation.setUserAgentOverride', {
    userAgent: (await cdp.eval('navigator.userAgent')),
    acceptLanguage,
  });
  await cdp.send('Emulation.setTimezoneOverride', { timezoneId: tz });
}

async function part3(cdp, port) {
  log('=== ③ 語言決定 ===');
  const base = `http://127.0.0.1:${port}`;
  const srv = startServer(port);
  try {
    if (!pre(await waitUp(port), `ui_preview 在 10 秒內開始回應；stderr：${srv.stderr().slice(0, 200)}`)) return;
    await setSaved(cdp, base, null);
    if (!pre(await loadPage(cdp, `${base}/`), '儀表板載入且 window.cockpitI18n 存在')) return;

    // --- resolveLang 純函式（在頁面內呼叫）---
    const got = await cdp.fn((cases) => cases.map((c) => window.cockpitI18n.resolveLang(c[1])), RESOLVE_CASES);
    RESOLVE_CASES.forEach((c, i) => check(got[i] === c[2], `③ resolveLang：${c[0]}（實際 ${got[i]}）`));
    check((await cdp.eval('window.cockpitI18n.resolveLang()')) === 'en', '③ resolveLang() 不帶參數不丟例外、回 en');

    // --- 端到端 ---
    for (const c of E2E_CASES) {
      await setEnv(cdp, c);
      await setSaved(cdp, base, c.saved);
      const okLoad = await loadPage(cdp, `${base}/`);
      pre(okLoad, `③ 情境「${c.name}」：頁面載入完成`);
      if (!okLoad) continue;
      const r = await cdp.eval(`(() => ({
        lang: window.cockpitI18n.lang,
        htmlLang: document.documentElement.getAttribute('lang'),
        navLangs: Array.from(navigator.languages),
        tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
        tab: document.getElementById('files-tab-changes').textContent,
        leftAria: document.querySelector('#files [role="tablist"]').getAttribute('aria-label'),
      }))()`);
      pre(r.navLangs[0] === c.acceptLanguage.split(',')[0] && r.tz === c.tz,
        `③ 情境「${c.name}」：環境覆寫生效（navigator.languages ${J(r.navLangs)}、時區 ${r.tz}）`);
      check(r.lang === c.lang && r.htmlLang === c.htmlLang,
        `③ 情境「${c.name}」：cockpitI18n.lang=${c.lang}、<html lang="${c.htmlLang}">（實際 ${r.lang}、${r.htmlLang}）`);
      check(r.tab === TAB_TEXT[c.lang] && r.leftAria === LEFT_ARIA[c.lang],
        `③ 情境「${c.name}」：靜態節點已套用該語言（分頁「${r.tab}」、aria-label「${r.leftAria}」）`);
    }

    // --- 預設繁中時，靜態文字與改動前逐字相同 ---
    await setEnv(cdp, { acceptLanguage: 'en-US', tz: 'Asia/Taipei' });
    await setSaved(cdp, base, null);
    await loadPage(cdp, `${base}/`);
    const zhStatic = await cdp.eval(`(() => ({
      files: document.getElementById('files-tab-files').textContent,
      changes: document.getElementById('files-tab-changes').textContent,
      empty: Array.from(document.querySelectorAll('#files-panel .files-empty, #changes-panel .files-empty')).map((n) => n.textContent),
      reviewAria: document.querySelector('#review [role="tablist"]').getAttribute('aria-label'),
    }))()`);
    check(zhStatic.files === '檔案' && zhStatic.changes === '變更' && zhStatic.reviewAria === '分頁'
      && zhStatic.empty.length === 2 && zhStatic.empty.every((t) => t === '先在 Factory Floor 或 runtime 清單選一個 pane'),
    `③ 繁中時靜態文字與改動前逐字相同（${J(zhStatic)}）`);

    // --- 第一次繪製前決定語言（階段審查 1 M1；spec「任何介面文字繪製之前決定語言」）---
    // 在文件開頭掛一個 DOMContentLoaded 監聽（早於 i18n.js 自己的，所以先跑）：此時 <body> 已解析完、靜態節點還是後備文字、
    // 翻譯尚未套用；英文介面這一刻 <html lang> 必須已是 en、i18n-pending 還在、靜態節點被藏起來（computed visibility: hidden），
    // 使用者不會看到繁中後備文字；載入完成後移除 class、節點可見且已是英文。繁中不藏。i18n.js 的 <script> 必須在 <head> 內。
    for (const c of [
      { name: '英文', acceptLanguage: 'en-US', tz: NY, saved: null, lang: 'en', htmlLang: 'en', after: 'Files' },
      { name: '繁中', acceptLanguage: 'zh-TW', tz: 'Asia/Taipei', saved: null, lang: 'zh', htmlLang: 'zh-Hant', after: '檔案' },
    ]) {
      await setEnv(cdp, c);
      await setSaved(cdp, base, c.saved);
      const { result } = await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
        source: `window.__i18nProbe = null;
          document.addEventListener('DOMContentLoaded', function () {
            var n = document.getElementById('files-tab-files');
            var s = document.querySelector('script[src="/app/i18n.js"]');
            window.__i18nProbe = {
              lang: document.documentElement.getAttribute('lang'),
              pending: document.documentElement.classList.contains('i18n-pending'),
              visibility: n ? getComputedStyle(n).visibility : null,
              text: n ? n.textContent : null,
              scriptParent: s && s.parentElement ? s.parentElement.tagName : null,
            };
          }, true);`,
      });
      const ok = await loadPage(cdp, `${base}/`);
      pre(ok, `③ 首次繪製前探針（${c.name}）：頁面載入完成`);
      if (ok) {
        const probe = await cdp.eval('window.__i18nProbe');
        const fin = await cdp.eval(`(() => { const n = document.getElementById('files-tab-files'); return { pending: document.documentElement.classList.contains('i18n-pending'), visibility: getComputedStyle(n).visibility, text: n.textContent }; })()`);
        const hidden = c.lang === 'en';
        check(probe && probe.scriptParent === 'HEAD', `③ [${c.name}] i18n.js 的 <script> 在 <head> 內（實際 ${J(probe && probe.scriptParent)}）`);
        check(probe && probe.lang === c.htmlLang && probe.pending === hidden && probe.visibility === (hidden ? 'hidden' : 'visible') && probe.text === '檔案',
          `③ [${c.name}] <body> 解析完、翻譯套用前：<html lang>=${c.htmlLang}、i18n-pending=${hidden}、靜態節點 visibility=${hidden ? 'hidden' : 'visible'}、仍是後備文字（實際 ${J(probe)}）`);
        check(fin.pending === false && fin.visibility === 'visible' && fin.text === c.after, `③ [${c.name}] 載入完成後移除 i18n-pending、靜態節點可見且為該語言（實際 ${J(fin)}）`);
      }
      await cdp.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: result.identifier });
    }

    // --- localStorage 存取丟例外視為沒有該項資訊 ---
    for (const [tz, accept, want] of [['Asia/Taipei', 'en-US', 'zh'], [NY, 'en-US', 'en'], [NY, 'zh-TW', 'zh']]) {
      await setEnv(cdp, { acceptLanguage: accept, tz });
      const { result } = await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
        source: `Object.defineProperty(window, 'localStorage', { configurable: true, get() { throw new DOMException('模擬：本機儲存不可用', 'SecurityError'); } });`,
      });
      const id = result.identifier;
      const ok = await loadPage(cdp, `${base}/`);
      const r = ok ? await cdp.eval(`(() => ({ lang: window.cockpitI18n.lang, thrown: (() => { try { window.localStorage; return false; } catch (e) { return true; } })() }))()`) : {};
      pre(ok && r.thrown === true, `③ localStorage 存取丟例外的模擬生效（${tz}、${accept}）`);
      check(ok && r.lang === want, `③ localStorage 丟例外時忽略它，由語言與時區決定：${accept}＋${tz} → ${want}（實際 ${r.lang}）`);
      await cdp.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: id });
    }

    // --- setLang：寫入 cockpit.lang 並重新載入 ---
    await setEnv(cdp, { acceptLanguage: 'en-US', tz: 'Asia/Taipei' });
    await setSaved(cdp, base, null);
    await loadPage(cdp, `${base}/`);
    pre((await cdp.eval('window.cockpitI18n.lang')) === 'zh', '③ setLang 前：Asia/Taipei 預設繁中');
    await cdp.eval('window.__oldDoc = true; undefined');
    await cdp.eval("window.cockpitI18n.setLang('en'); undefined");
    const reloaded = await cdp.waitFor("!window.__oldDoc && document.readyState === 'complete' && !!window.cockpitI18n", 15000);
    const after = reloaded ? await cdp.eval(`({ lang: window.cockpitI18n.lang, saved: localStorage.getItem('cockpit.lang'), tab: document.getElementById('files-tab-changes').textContent })`) : {};
    check(reloaded && after.saved === 'en' && after.lang === 'en' && after.tab === 'Changes',
      `③ setLang('en') 寫入 cockpit.lang 並重新載入後為英文（${J(after)}）`);
    await cdp.eval("window.cockpitI18n.setLang('xx'); undefined");
    await sleep(500);
    check((await cdp.eval("localStorage.getItem('cockpit.lang')")) === 'en' && (await cdp.eval('window.cockpitI18n.lang')) === 'en',
      '③ setLang 傳入非 zh／en 時不動作（不寫入、不重新載入）');
    check(cdp.exceptions.length === 0, `③ 整段沒有未捕捉的頁面例外（${J(cdp.exceptions.slice(0, 3))}）`);
  } finally {
    try {
      await cdp.send('Emulation.setTimezoneOverride', { timezoneId: '' });
      await cdp.send('Page.navigate', { url: 'about:blank' });
    } catch {
      // 瀏覽器已關。
    }
    await sleep(300);
    killTree(srv.proc, 'ui_preview（③）');
  }
}

// ---------------------------------------------------------------------------
// ④ 切換按鈕與兩個視窗同步
// ---------------------------------------------------------------------------

// 開同一個 Chrome 的第二個分頁（同源、同 user-data-dir，storage 事件才會跨分頁送達）。
async function openSecondPage(cdpPort) {
  const r = await fetch(`http://127.0.0.1:${cdpPort}/json/new?about:blank`, { method: 'PUT' });
  const target = await r.json();
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = rej;
  });
  const cdp = new CDP(ws);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  return { cdp, ws, id: target.id, cdpPort };
}

async function closeSecondPage(h) {
  if (!h) return;
  try {
    h.ws.close();
  } catch {
    // 已斷線。
  }
  try {
    await fetch(`http://127.0.0.1:${h.cdpPort}/json/close/${h.id}`);
  } catch {
    // 已關。
  }
}

const BTN_SEL = 'button.lang-toggle[data-action="toggle-language"]';
const BTN_STATE = `(() => {
  const b = document.querySelector('${BTN_SEL}');
  if (!b) return null;
  const bell = document.querySelector('.notify-bell');
  const rb = b.getBoundingClientRect();
  const rl = bell ? bell.getBoundingClientRect() : null;
  return {
    text: b.textContent, lang: b.getAttribute('lang'), aria: b.getAttribute('aria-label'), disabled: b.disabled,
    title: b.getAttribute('title'), inTopbar: !!b.closest('#topbar'), afterBell: !!bell && bell.nextElementSibling === b,
    sameHeight: !!rl && Math.abs(rb.height - rl.height) <= 1, sameCenter: !!rl && Math.abs((rb.top + rb.height / 2) - (rl.top + rl.height / 2)) <= 1,
    x: rb.left + rb.width / 2, y: rb.top + rb.height / 2, w: rb.width,
    htmlLang: document.documentElement.getAttribute('lang'), saved: (() => { try { return localStorage.getItem('cockpit.lang'); } catch (e) { return 'ERR'; } })(),
    i18nLang: window.cockpitI18n && window.cockpitI18n.lang,
  };
})()`;

async function waitButton(cdp, timeoutMs = 10000) {
  return cdp.waitFor(`!!document.querySelector('${BTN_SEL}')`, timeoutMs);
}

async function mouseClick(cdp, x, y) {
  for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased']) {
    await cdp.send('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mousePressed' ? 1 : 0, clickCount: 1 });
  }
}

async function pressEnter(cdp) {
  const base = { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 };
  await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', text: '\r', ...base });
  await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', ...base });
}

// 以真的 Tab 鍵把焦點移到 sel（程式 focus() 在先前有滑鼠操作的頁面上不算 :focus-visible，不能代表鍵盤使用者）。
async function tabTo(cdp, sel, max = 40) {
  const key = { key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9, nativeVirtualKeyCode: 9 };
  for (let i = 0; i < max; i += 1) {
    if (await cdp.eval(`document.activeElement === document.querySelector(${J(sel)})`)) return true;
    await cdp.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', ...key });
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', ...key });
  }
  return cdp.eval(`document.activeElement === document.querySelector(${J(sel)})`);
}

// 標記舊文件、之後等「新文件載入完且 i18n 就緒」。
const markOld = (cdp) => cdp.eval('window.__oldDoc = true; undefined');
const waitReloaded = (cdp, ms = 15000) =>
  cdp.waitFor("!window.__oldDoc && document.readyState === 'complete' && !!window.cockpitI18n", ms);

async function part4(cdp, port, chrome) {
  log('=== ④ 切換按鈕與兩個視窗同步 ===');
  const base = `http://127.0.0.1:${port}`;
  const srv = startServer(port);
  let second = null;
  try {
    if (!pre(await waitUp(port), `ui_preview 在 10 秒內開始回應；stderr：${srv.stderr().slice(0, 200)}`)) return;
    // 台灣時區、沒有 cockpit.lang → 預設繁中。
    await setEnv(cdp, { acceptLanguage: 'en-US', tz: 'Asia/Taipei' });
    await setSaved(cdp, base, null);
    if (!pre(await loadPage(cdp, `${base}/`), '儀表板載入且 window.cockpitI18n 存在')) return;
    const hasBtn = await waitButton(cdp);
    if (!check(hasBtn, '④ 頂列出現語言切換按鈕')) return;

    // --- 繁中介面的按鈕 ---
    let s = await cdp.eval(BTN_STATE);
    check(s && s.inTopbar && s.afterBell, '④ 按鈕在頂列、緊接在通知鈴鐺之後');
    check(s && s.text === 'EN' && s.lang === 'en' && s.aria === 'Switch to English' && s.disabled === false && s.title === null,
      `④ 繁中介面：文字 EN、lang="en"、可及名稱 Switch to English、可點（${J(s)}）`);
    check(s && s.htmlLang === 'zh-Hant' && s.i18nLang === 'zh', '④ 繁中介面：<html lang="zh-Hant">');
    check(s && s.sameHeight && s.sameCenter, '④ 按鈕與鈴鐺等高且垂直置中對齊（不撐高頂列）');
    check((await cdp.eval("document.querySelector('.notify-bell').getAttribute('aria-label')")) === '通知設定', '④ 鈴鐺沒被改動（aria-label 仍為「通知設定」）');
    // 整頁重畫後仍在（投影每 100ms 推一次）。
    await sleep(600);
    check((await cdp.eval(BTN_STATE)) !== null, '④ 重畫幾次之後按鈕仍在');

    // --- 滑鼠點擊 → 英文 ---
    await markOld(cdp);
    await mouseClick(cdp, s.x, s.y);
    check(await waitReloaded(cdp), '④ 點擊後頁面重新載入');
    await waitButton(cdp);
    s = await cdp.eval(BTN_STATE);
    check(s && s.saved === 'en' && s.i18nLang === 'en' && s.htmlLang === 'en', `④ 點擊切換為英文：cockpit.lang=en、<html lang="en">（${J([s && s.saved, s && s.htmlLang])}）`);
    check(s && s.text === '中文' && s.lang === 'zh-Hant' && s.aria === '切換為繁體中文' && s.disabled === false,
      `④ 英文介面：文字 中文、lang="zh-Hant"、可及名稱 切換為繁體中文（${J(s)}）`);
    check(s && s.afterBell && s.sameHeight && s.sameCenter, '④ 英文介面：按鈕仍緊接鈴鐺、與鈴鐺等高對齊');

    // --- 鍵盤：焦點可見，Enter 切回繁中 ---
    await tabTo(cdp, BTN_SEL);
    const fo = await cdp.eval(`(() => { const b = document.querySelector('${BTN_SEL}'); const cs = getComputedStyle(b);
      return { active: document.activeElement === b, fv: b.matches(':focus-visible'), style: cs.outlineStyle, width: cs.outlineWidth }; })()`);
    check(fo.active && fo.fv && fo.style === 'solid' && parseFloat(fo.width) >= 2, `④ 鍵盤焦點在按鈕上時有可見外框（${J(fo)}）`);
    await markOld(cdp);
    await pressEnter(cdp);
    check(await waitReloaded(cdp), '④ 鍵盤 Enter 後頁面重新載入');
    await waitButton(cdp);
    s = await cdp.eval(BTN_STATE);
    check(s && s.saved === 'zh' && s.i18nLang === 'zh' && s.htmlLang === 'zh-Hant' && s.text === 'EN', `④ Enter 切回繁中（${J(s)}）`);

    // --- 兩個視窗同步：兩個分頁都在英文，在 A 按切換，B 跟著重新載入顯示繁中 ---
    await setSaved(cdp, base, 'en');
    pre(await loadPage(cdp, `${base}/`), '④ 視窗 A 以英文載入');
    await waitButton(cdp);
    second = await openSecondPage(chrome.cdpPort);
    const b = second.cdp;
    await b.send('Page.navigate', { url: `${base}/` });
    pre(await b.waitFor("document.readyState === 'complete' && !!window.cockpitI18n", 15000), '④ 視窗 B 載入');
    await waitButton(b);
    const bs0 = await b.eval(BTN_STATE);
    pre(bs0 && bs0.i18nLang === 'en' && bs0.text === '中文', `④ 視窗 B 也是英文（${J(bs0)}）`);
    // 無關的 storage 鍵不會讓 B 重載。
    await markOld(b);
    await cdp.eval("localStorage.setItem('cockpit.other-key', 'x'); undefined");
    await sleep(800);
    check((await b.eval('window.__oldDoc === true')) === true, '④ 別的 storage 鍵變動時視窗 B 不重新載入');
    // 在 A 按切換。
    await markOld(b);
    const as = await cdp.eval(BTN_STATE);
    await mouseClick(cdp, as.x, as.y);
    const bReloaded = await waitReloaded(b);
    check(bReloaded, '④ 在視窗 A 按切換後視窗 B 也重新載入');
    await waitButton(b);
    const bs1 = await b.eval(BTN_STATE);
    check(bs1 && bs1.i18nLang === 'zh' && bs1.htmlLang === 'zh-Hant' && bs1.text === 'EN', `④ 視窗 B 重新載入後顯示繁中（${J(bs1)}）`);
    await closeSecondPage(second);
    second = null;
    await cdp.eval("localStorage.removeItem('cockpit.other-key'); undefined");

    // --- localStorage 的 setItem 丟例外：按鈕停用、有 title、點了不動作 ---
    for (const [saved, text, title, aria] of [
      ['zh', 'EN', '需要瀏覽器儲存空間才能切換語言', 'Switch to English'],
      ['en', '中文', 'Switching language needs browser storage', '切換為繁體中文'],
    ]) {
      await setSaved(cdp, base, saved);
      const { result } = await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
        source: "Storage.prototype.setItem = function () { throw new DOMException('模擬：本機儲存不可用', 'QuotaExceededError'); };",
      });
      const ok = await loadPage(cdp, `${base}/`);
      const gotBtn = ok && (await waitButton(cdp));
      const d = gotBtn ? await cdp.eval(BTN_STATE) : null;
      const throws = ok ? await cdp.eval("(() => { try { localStorage.setItem('x', '1'); return false; } catch (e) { return true; } })()") : false;
      pre(throws === true, `④ setItem 丟例外的模擬生效（cockpit.lang=${saved}）`);
      check(d && d.disabled === true && d.title === title && d.text === text && d.aria === aria,
        `④ ${saved === 'zh' ? '繁中' : '英文'}介面、儲存不可用：按鈕停用且 title=「${title}」（${J(d)}）`);
      if (d) {
        await markOld(cdp);
        await mouseClick(cdp, d.x, d.y);
        await cdp.eval(`document.querySelector('${BTN_SEL}').focus(); undefined`);
        await pressEnter(cdp); // disabled 的按鈕本來就不能被聚焦；focus() 沒作用也不影響「不動作」的斷言
        await sleep(700);
        const after = await cdp.eval('({ old: window.__oldDoc === true, saved: (() => { try { return localStorage.getItem("cockpit.lang"); } catch (e) { return "ERR"; } })() })');
        check(after.old === true && after.saved === saved, `④ 停用的按鈕點擊與 Enter 都不動作（沒重載、cockpit.lang 仍為 ${saved}；${J(after)}）`);
      }
      await cdp.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: result.identifier });
    }
    // 拿掉覆寫後恢復可點。
    await setSaved(cdp, base, 'zh');
    await loadPage(cdp, `${base}/`);
    await waitButton(cdp);
    const back = await cdp.eval(BTN_STATE);
    check(back && back.disabled === false && back.title === null, '④ 儲存恢復可用後按鈕為啟用、沒有 title');
    check(cdp.exceptions.length === 0, `④ 整段沒有未捕捉的頁面例外（${J(cdp.exceptions.slice(0, 3))}）`);
  } finally {
    await closeSecondPage(second);
    try {
      await cdp.send('Emulation.setTimezoneOverride', { timezoneId: '' });
      await cdp.send('Page.navigate', { url: 'about:blank' });
    } catch {
      // 瀏覽器已關。
    }
    await sleep(300);
    killTree(srv.proc, 'ui_preview（④）');
  }
}

// ---------------------------------------------------------------------------
// ② 英文介面沒有繁中字典字串（task 2.1：render.js／actions.js／output.js 的畫面；之後的 task 補其他模組）
// ---------------------------------------------------------------------------

// 含 CJK 標點（U+3000–U+303F：「」、。等）、表意文字（U+3400–U+9FFF）與全形（U+FF00–U+FFEF）；英文介面不得出現其中任何一個。
const CJK_RE = /[　-〿㐀-鿿＀-￯]/;
const CJK_RUN_RE = /[一-鿿]{4,}/;
const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// 使用者資料（兩種語言都照原文：project／stage／task／workstream 名稱、pane 標題與 cwd、事件內容、pane 輸出）與
// 後端原文欄位（連線 reason、protocol 警告、project 警告——task 3.3 才改成代碼翻譯；本 task 照原文）不參與比對。
// 注意 `.event-detail` 不在此列：事件說明若是 Cockpit 訊息（drift 的原因）要依代碼翻譯（設計審核），英文介面不得有繁中。
const USER_DATA_SEL = [
  '.task-title', '.ff-ws-name', '.ff-stage-name', '.project-name', '.project-item-name', '.pane-title', '.pane-cwd',
  '.pane-id', '.pane-agent', '.workspace-label', '.runtime-id', '.runtime-lamp-id', '.event-at', '.event-runtime',
  '.event-kind', '.event-subject', '.ff-binding-runtime', '.ff-binding-pane', '.output-title', '.output-text',
  // files.js／viewers.js（task 2.2）：根目錄名稱、runtime id、檔案與資料夾名稱、分頁上的檔名、工具列的相對路徑。
  // files.js 檔案樹表頭的 runtime id、git.js「變更」面板表頭的 runtime id（第 2 個 span；第 3 個 span 是分支資訊，含「分離 HEAD」
  // 等介面字串，要受檢查）。
  '.files-root-name', '#files-panel .files-runtime', '#changes-panel .files-head-names > .files-runtime:nth-of-type(2)',
  '.tree-name', '.review-tab-label', '.file-toolbar-path',
  // git.js（task 2.3）：commit 的標題／作者／時間／hash／ref 名稱、分支篩選的 ref 名稱、變更清單的目錄與狀態字母、diff 的路徑與格線內容、
  // commit 詳情的 hash 與訊息本文。
  '.graph-subject', '.graph-author', '.graph-time', '.graph-hash', '.graph-ref-badge', '.graph-filter-item', '.changes-dir',
  '.changes-status', '.diff-toolbar-path', '.diff-text', '.diff-num', '.commit-detail-hash', '.commit-detail-message',
].join(',');
// 檔案內容（Markdown 渲染結果、純文字）是使用者資料，但裡面帶的 title／aria-label 是介面字串，所以只排除文字節點、屬性照常檢查。
const USER_CONTENT_TEXT_SEL = ['.md-body', '.text-content'].join(',');
// task 3.3 起連線原因、protocol 警告、project 警告都依代碼翻譯，不再排除：英文介面下它們也受「沒有繁中字典字串、沒有殘留 CJK」檢查。
// 仍是原樣顯示的只有 HERDR／作業系統回傳的原文（代碼 raw），範例資料沒有這種情況；它們的行為由下面的專段（msgState）逐字斷言。
// task 2.4 起所有模組都已進字典，沒有「待處理模組」的排除清單（原 PENDING_MODULE_SEL 已刪除）。
const SKIP_SEL = USER_DATA_SEL;
// 殘留 CJK 檢查另外放行：語言切換按鈕本來就用目標語言寫（英文介面顯示「中文」，spec「語言切換按鈕」）；
// Markdown 標題後的錨點連結 aria-label 帶標題原文（使用者資料；字典外框另由 EXPECT 逐字比）。
const RESIDUE_SKIP_BASE = '.lang-toggle,.md-body a.anchor';
// 殘留 CJK 檢查的範圍：#app、Live Output（#output 在 #review 內）、檔案樹、「變更」面板與下半部分頁區（檔案、diff、Git Graph、某版本）、
// 通知設定面板（body 底下、#app 之外；task 2.4）。
const RESIDUE_SCOPE_SEL = '#app, #files-panel, #changes-panel, #review, #notify-panel';

// 頁面內：收集所有文字節點與 title／aria-label／placeholder 屬性值（含 hidden 節點）。
// skipSel 命中的元素（含祖先）標 skipped（兩項檢查都排除）；inApp 表示在 #app 或 #output 之內，且不在 residueSkipSel
// 命中的元素內（只用於殘留 CJK 檢查的範圍；例如錯誤 banner 的後端原因）。
function collectUi(skipSel, residueSkipSel, textOnlySkipSel, scopeSel) {
  const out = [];
  const where = (el) => `${el.tagName.toLowerCase()}${el.id ? '#' + el.id : ''}${el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : ''}`;
  const add = (kind, text, el) => {
    out.push({
      kind, text, where: where(el), skipped: !!el.closest(skipSel) || (kind === 'text' && !!el.closest(textOnlySkipSel)),
      inApp: !!el.closest(scopeSel) && !(residueSkipSel && el.closest(residueSkipSel)),
    });
  };
  const walker = document.createTreeWalker(document.documentElement, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const p = n.parentElement;
    if (!p || ['SCRIPT', 'STYLE', 'NOSCRIPT'].includes(p.tagName)) continue;
    const text = n.textContent.trim();
    if (text) add('text', text, p);
  }
  for (const el of document.querySelectorAll('*')) {
    for (const attr of ['title', 'aria-label', 'placeholder']) {
      const v = el.getAttribute(attr);
      if (v && v.trim()) add(attr, v.trim(), el);
    }
  }
  return out;
}

// 頁面內：依 { key: [選擇器, 屬性或 null] } 取出各選擇器所有命中的文字（null＝textContent）或屬性值。
function probeUi(map) {
  const got = {};
  for (const [key, [sel, attr]] of Object.entries(map)) {
    got[key] = Array.from(document.querySelectorAll(sel)).map((el) => (attr ? el.getAttribute(attr) : el.textContent.trim()));
  }
  return got;
}

// 從繁中字典值建立比對器（規則見檔頭 ②）：值裡沒有任何 CJK 字元的（例如 `HTTP {status}`、`EN`）不可能是繁中字串，略過。
// (a) 整段 trim 後的文字與「去掉佔位符的整個值」完全相等；(b) 固定片段（以佔位符切開）含 ≥ 4 個連續 CJK 字元者，
// 文字只要包含該片段；(a') 比檢查檔頭規則更嚴的一條：整段文字符合「佔位符當萬用字元」的範本（抓 `警告 3`、`歧義（2）` 這類
// 短於 4 個 CJK 字元、帶資料的句子）。
function buildZhMatchers(zh) {
  const matchers = [];
  for (const [key, value] of Object.entries(zh)) {
    if (!CJK_RE.test(value)) continue;
    const frags = value.split(/\{\w+\}/);
    const stripped = frags.join('').trim();
    const template = new RegExp(`^${value.split(/\{\w+\}/).map(escapeRe).join('[\\s\\S]*')}$`);
    const longFrags = frags.map((f) => f.trim()).filter((f) => CJK_RUN_RE.test(f));
    matchers.push({ key, stripped, template, longFrags });
  }
  return matchers;
}

function zhHits(matchers, text) {
  const hits = [];
  for (const m of matchers) {
    if (m.stripped && text === m.stripped) hits.push({ key: m.key, rule: 'a' });
    else if (m.template.test(text)) hits.push({ key: m.key, rule: "a'" });
    else if (m.longFrags.some((f) => text.includes(f))) hits.push({ key: m.key, rule: 'b' });
  }
  return hits;
}

// 掃描目前頁面（英文介面）：不含繁中字典字串，且 #app／#output 沒有殘留的 CJK 文字。
// pageExpr（選填）：要在同一個同步 eval 裡先做的事（例如餵一份空投影），回傳值放進 got。
async function scanEnglish(cdp, matchers, name, { residueSkip = '', pageExpr = '', probeMap = null } = {}) {
  const expr = `(() => {
    const collect = ${collectUi.toString()};
    const probe = ${probeUi.toString()};
    ${pageExpr}
    return { items: collect(${J(SKIP_SEL)}, ${J([RESIDUE_SKIP_BASE, residueSkip].filter(Boolean).join(','))}, ${J(USER_CONTENT_TEXT_SEL)}, ${J(RESIDUE_SCOPE_SEL)}), got: ${probeMap ? `probe(${J(probeMap)})` : 'null'} };
  })()`;
  const { items, got } = await cdp.eval(expr);
  pre(items.length > 20, `② ${name}：收集到畫面文字（${items.length} 筆）`);
  const live = items.filter((it) => !it.skipped);
  const hits = live.flatMap((it) => zhHits(matchers, it.text).map((h) => ({ ...h, kind: it.kind, text: it.text.slice(0, 60), where: it.where })));
  check(hits.length === 0, `② ${name}：英文介面沒有繁中字典字串（命中 ${J(hits.slice(0, 4))}）`);
  const residue = live.filter((it) => it.inApp && CJK_RE.test(it.text)).map((it) => ({ kind: it.kind, text: it.text.slice(0, 60), where: it.where }));
  check(residue.length === 0, `② ${name}：#app／#output／檔案樹／檔案分頁沒有殘留的 CJK 文字（使用者資料與後端原文欄位除外；殘留 ${J(residue.slice(0, 4))}）`);
  return { got, items };
}

// 各畫面要讀的節點：{ 名稱: [選擇器, 屬性或 null] }。
const PROBE_MAP = {
  lampStale: ['.topbar .runtime-lamp-stale', null],
  lampTitleWin: ['.runtime-lamp[data-runtime="win"]', 'title'],
  lampTitleWsl: ['.runtime-lamp[data-runtime="wsl"]', 'title'],
  bellLabel: ['.notify-bell', 'aria-label'],
  bellTitle: ['.notify-bell', 'title'],
  warnCount: ['.project-item-warnings', null],
  connStale: ['.runtime-conn-stale', null],
  bindText: ['.ff-binding-text', null],
  bindBadge: ['.ff-binding-badge', null],
  taskBtns: ['.task-actions .action-button', null],
  rowBtns: ['.ff-row-actions .action-button', null],
  focusMark: ['.pane-herdr-focus', null],
  focusTitle: ['.pane-herdr-focus', 'title'],
  undeclared: ['.ff-undeclared', null],
  eventsTitle: ['.recent-events-title', null],
  eventDetail: ['.event-detail', null],
  chanLabel: ['.statusbar-channel-label', null],
  chanTitle: ['.statusbar-channel', 'title'],
  outEmpty: ['.output-empty', null],
  outStale: ['.output-stale-label', null],
  outClose: ['.output-close', null],
  outTrunc: ['.output-truncated-notice', null],
  outGone: ['.output-gone-notice', null],
  outReason: ['.output-error-reason', null],
  rebindBanner: ['.rebind-banner .action-banner-text', null],
  rebindCancel: ['.rebind-banner .action-button', null],
  bindHere: ['.pane-row .action-button', null],
  errBanner: ['.error-banner .action-banner-text', null],
  errClose: ['.error-banner .action-button', null],
  projEmpty: ['.projects-empty-state', null],
  floorEmpty: ['.floor-empty-state', null],
  connDd: ['.connection-details dd', null],
  projWarn: ['.project-warning', null],
  protoWarn: ['.protocol-warning', null],
};

// 每條期望：[快照名稱, probe 鍵, { en, zh }, 'eq'（整份陣列相等）或 'has'（含這些值）]。繁中值是改動前的字串逐字，
// 與字典無關——所以繁中介面這一段是「沒有任何改變」的獨立檢查。
const FLOOR_EMPTY = {
  en: 'Add a [[project]] section to cockpit.toml to see the Factory Floor here. Restart cockpit after adding it.',
  zh: '在 cockpit.toml 加入 [[project]] 區段即可在這裡看到 Factory Floor，加入後需要重啟 cockpit',
};
const EXPECT = [
  ['default', 'lampStale', { en: ['Last known', 'Last known'], zh: ['最後已知', '最後已知'] }, 'eq'],
  ['default', 'lampTitleWin', { en: ['cockpit → HERDR runtime win: connected'], zh: ['cockpit → HERDR runtime win：connected'] }, 'eq'],
  ['default', 'lampTitleWsl', { en: ['cockpit → HERDR runtime wsl: disconnected'], zh: ['cockpit → HERDR runtime wsl：disconnected'] }, 'eq'],
  ['default', 'bellLabel', { en: ['Notification settings'], zh: ['通知設定'] }, 'eq'],
  ['default', 'bellTitle', { en: ['Notification settings'], zh: ['通知設定'] }, 'eq'],
  ['default', 'warnCount', { en: ['1 warning'], zh: ['警告 1'] }, 'eq'],
  ['default', 'connStale', { en: ['Last known', 'Last known'], zh: ['最後已知', '最後已知'] }, 'eq'],
  ['default', 'bindText', { en: ['Runtime disconnected', 'No binding', 'Unbound'], zh: ['runtime 未連線', '無綁定', '未綁定'] }, 'has'],
  ['default', 'bindBadge', { en: ['Rebound'], zh: ['改綁'] }, 'has'],
  ['default', 'taskBtns', { en: ['Back', 'Advance', 'Completed', 'Failed', 'Clear mark'], zh: ['退回', '推進', 'Completed', 'Failed', '清除標記'] }, 'has'],
  ['default', 'rowBtns', { en: ['View output', 'Rebind', 'Undo rebind'], zh: ['看輸出', '改綁', '取消改綁'] }, 'has'],
  ['default', 'focusMark', { en: ['Active'], zh: ['作用中'] }, 'eq'],
  ['default', 'focusTitle', { en: ['Pane currently focused in HERDR'], zh: ['HERDR 目前聚焦的 pane'] }, 'eq'],
  ['default', 'eventsTitle', { en: ['Recent events'], zh: ['最近事件'] }, 'eq'],
  // 設計審核：最近事件的 drift 說明依 detail_msg 翻譯；HERDR 產生的說明（agent 狀態、exit code）兩種語言都照原文。
  ['default', 'eventDetail', {
    en: ['working', 'exit code 0', 'WSL distro Ubuntu-24.04 is not running'],
    zh: ['working', 'exit code 0', 'WSL 發行版 Ubuntu-24.04 未啟動'],
  }, 'eq'],
  ['default', 'chanLabel', { en: ['cockpit service'], zh: ['cockpit 服務'] }, 'eq'],
  ['default', 'chanTitle', { en: ['Browser → cockpit service: connected'], zh: ['瀏覽器 → cockpit 服務：connected'] }, 'eq'],
  ['default', 'outEmpty', {
    en: ['No pane selected. Click any row in the runtime list, or press "View output" on a Factory Floor row.'],
    zh: ['還沒選 pane。點 runtime 清單裡的任一列，或按 Factory Floor 列首的「看輸出」。'],
  }, 'eq'],
  ['default', 'outStale', { en: ['Stale'], zh: ['過期'] }, 'eq'],
  ['default', 'outClose', { en: ['Deselect'], zh: ['取消選取'] }, 'eq'],
  ['default', 'outTrunc', { en: ['Earlier output not shown'], zh: ['更早的輸出未顯示'] }, 'eq'],
  ['default', 'outGone', { en: ['Pane no longer exists'], zh: ['pane 已不存在'] }, 'eq'],
  // task 3.3：WSL 未啟動的連線原因依代碼翻譯（含發行版名稱）；project 警告依代碼翻譯。繁中介面顯示後端原文（範例資料的警告原文是手寫樣本）。
  ['default', 'connDd', { en: ['WSL distro Ubuntu-24.04 is not running'], zh: ['WSL 發行版 Ubuntu-24.04 未啟動'] }, 'has'],
  ['default', 'projWarn', {
    en: ['Task docs-1: stage "Draft" is no longer in the pipeline stages; reset to the start stage "Spec"'],
    zh: ['task docs-1 的 stage "Draft" 已不在 stages，退回起始 stage'],
  }, 'eq'],
  ['default', 'protoWarn', { en: [], zh: [] }, 'eq'],
  ['projectP', 'bindText', { en: ['Ambiguous (2)'], zh: ['歧義（2）'] }, 'has'],
  ['projectP', 'undeclared', { en: ['Working, no task declared'], zh: ['工作中・未宣告 task'] }, 'eq'],
  ['rebind', 'rebindBanner', {
    en: ['Rebind mode: pick a pane for AI Cockpit / Backend, then click "Bind here" on its row'],
    zh: ['改綁模式：為 AI Cockpit / Backend 選一個 pane，按該列的「綁定到這裡」'],
  }, 'eq'],
  ['rebind', 'rebindCancel', { en: ['Cancel'], zh: ['取消'] }, 'eq'],
  ['rebind', 'bindHere', { en: ['Bind here'], zh: ['綁定到這裡'] }, 'has'],
  ['errHttp', 'errBanner', {
    en: ['Action failed (HTTP 409, POST /api/projects/cockpit/tasks/be-1/fail): ui_preview 模擬回應 409'],
    zh: ['操作失敗（HTTP 409，POST /api/projects/cockpit/tasks/be-1/fail）：ui_preview 模擬回應 409'],
  }, 'eq'],
  ['errHttp', 'errClose', { en: ['Close'], zh: ['關閉'] }, 'eq'],
  ['errNet', 'errBanner', {
    en: ['Action failed (request did not complete, POST /api/projects/cockpit/tasks/be-1/advance): boom'],
    zh: ['操作失敗（請求沒有完成，POST /api/projects/cockpit/tasks/be-1/advance）：boom'],
  }, 'eq'],
  ['empty', 'projEmpty', { en: ['No Projects'], zh: ['沒有 Project'] }, 'eq'],
  ['empty', 'floorEmpty', { en: [FLOOR_EMPTY.en], zh: [FLOOR_EMPTY.zh] }, 'eq'],
  ['truncated', 'outTrunc', { en: ['Earlier output not shown'], zh: ['更早的輸出未顯示'] }, 'eq'],
  ['netError', 'outReason', { en: ["Can't reach the server, retrying"], zh: ['無法連線到伺服器，正在重試'] }, 'eq'],
  ['netError', 'outStale', { en: ['Stale'], zh: ['過期'] }, 'eq'],
  ['http502', 'outReason', { en: ['HTTP 502'], zh: ['HTTP 502'] }, 'eq'],
  ['gone', 'outGone', { en: ['Pane no longer exists'], zh: ['pane 已不存在'] }, 'eq'],
  ['timeout', 'outReason', { en: ['Request timed out (no response in 6 s), retrying'], zh: ['請求逾時（超過 6 秒沒有回應），正在重試'] }, 'eq'],
  ['chanDown', 'chanTitle', { en: ['Browser → cockpit service: disconnected'], zh: ['瀏覽器 → cockpit 服務：disconnected'] }, 'eq'],
  ['chanDown', 'lampTitleWin', { en: ['cockpit → HERDR runtime win: connected (last known)'], zh: ['cockpit → HERDR runtime win：connected（最後已知）'] }, 'eq'],
];

const sameList = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);

function expectSnapshot(L, snaps, name, table = EXPECT) {
  for (const [snap, key, want, mode] of table) {
    if (snap !== name) continue;
    const got = (snaps[name] && snaps[name][key]) || [];
    const wantList = want[L];
    let ok;
    if (mode === 'eq') ok = sameList(got, wantList);
    else if (mode === 'match') ok = got.length === wantList.length && got.every((g, i) => typeof g === 'string' && wantList[i].test(g));
    else ok = wantList.every((w) => got.includes(w));
    check(ok, `② [${L}] ${name}／${key}：${{ eq: '等於', has: '含', match: '符合' }[mode]} ${wantList.map(String).join(' | ')}（實際 ${J(got)}）`);
  }
}

const clickSel = (cdp, sel) => cdp.eval(`(() => { const el = document.querySelector(${J(sel)}); if (!el) return false; el.click(); return true; })()`);
const waitSel = (cdp, sel, ms = 10000) => cdp.waitFor(`!!document.querySelector(${J(sel)})`, ms);

// 一種語言走一遍 render.js／actions.js／output.js 的各個畫面。L 為 'en' 時另掃描「繁中字典字串」與殘留 CJK；
// 兩種語言都比對 EXPECT 的期望字串。server＝ui_preview 預設輸出模式＋ be-1/fail 回 409；Live Output 的失敗狀態以換掉頁面的 fetch 提供。
async function runUiScenario(cdp, matchers, baseA, L) {
  const en = L === 'en';
  const snaps = {};
  const snap = async (name, opts = {}) => {
    if (en) {
      const r = await scanEnglish(cdp, matchers, `${name}`, { ...opts, probeMap: PROBE_MAP });
      snaps[name] = r.got;
    } else {
      snaps[name] = await cdp.fn(probeUi, PROBE_MAP);
    }
    expectSnapshot(L, snaps, name);
  };
  const open = async (base) => {
    await setSaved(cdp, base, L);
    const ok = await loadPage(cdp, `${base}/`);
    pre(ok, `② [${L}] 儀表板載入（${base}）`);
    return ok && (await cdp.waitFor("!!document.querySelector('.runtime-lamp') && !!document.querySelector('.ff-binding')", 10000));
  };

  // Factory Floor 非 bound 的綁定文字（Unbound／Ambiguous／Runtime disconnected／No binding）單行省略，英文較長會被截斷：
  // 外層 .ff-binding 的 title 必須是同一段字典文字（階段審查 1 M3）。
  const bindTitleCheck = async (name) => {
    const rows = await cdp.eval(`Array.from(document.querySelectorAll('.ff-binding:not(.ff-binding-bound)')).map((w) => [w.getAttribute('title'), (w.querySelector('.ff-binding-text') || {}).textContent])`);
    check(rows.length > 0 && rows.every(([title, text]) => typeof title === 'string' && title !== '' && title === text),
      `② [${L}] ${name}：非 bound 的綁定文字都有 title，且等於顯示的文字（${rows.length} 筆；${J(rows.slice(0, 4))}）`);
    return rows;
  };

  if (!pre(await open(baseA), `② [${L}] 首份投影畫完`)) return;
  await snap('default');
  const defaultRows = await bindTitleCheck('default');
  check(defaultRows.some(([title]) => title === (en ? 'Runtime disconnected' : 'runtime 未連線')), `② [${L}] default：Runtime disconnected 的 title 是全文（${J(defaultRows)}）`);

  // 左欄切到 Project p（歧義綁定、未宣告 task）。
  pre(await clickSel(cdp, '[data-action="select-project"][data-project="p"]'), `② [${L}] 點左欄 Project p`);
  await cdp.waitFor("!!document.querySelector('.ff-undeclared')", 5000);
  await snap('projectP');
  const ambRows = await bindTitleCheck('projectP');
  check(ambRows.some(([title]) => title === (en ? 'Ambiguous (2)' : '歧義（2）')), `② [${L}] projectP：歧義綁定的 title 是全文（${J(ambRows)}）`);
  await clickSel(cdp, '[data-action="select-project"][data-project="cockpit"]');
  await cdp.waitFor("!!document.querySelector('[data-action=\"fail\"][data-task=\"be-1\"]')", 5000);

  // 改綁模式。
  pre(await clickSel(cdp, '.ff-row-actions [data-action="rebind"][data-workstream="be"]'), `② [${L}] 按 Backend 的「改綁」`);
  await waitSel(cdp, '.rebind-banner');
  await snap('rebind');
  await clickSel(cdp, '[data-action="rebind-cancel"]');

  // 錯誤 banner：409（ui_preview 的寫入規則）與請求沒有完成（把 fetch 換成 reject）。
  pre(await clickSel(cdp, '[data-action="fail"][data-task="be-1"]'), `② [${L}] 按 be-1 的 Failed`);
  await waitSel(cdp, '.error-banner');
  await snap('errHttp', { residueSkip: '.error-banner .action-banner-text' }); // ui_preview 的 409 本體沒有 code，原因照原文顯示（繁中）；外框用 EXPECT 逐字比，依代碼翻譯的情況見下面的專段
  await clickSel(cdp, '[data-action="error-dismiss"]');
  await cdp.eval("window.__origFetch = window.fetch; window.fetch = () => Promise.reject(new Error('boom')); undefined");
  pre(await clickSel(cdp, '[data-action="advance"][data-task="be-1"]'), `② [${L}] 按 be-1 的推進（fetch 被換成 reject）`);
  await waitSel(cdp, '.error-banner');
  await snap('errNet');
  await cdp.eval('window.fetch = window.__origFetch; undefined');
  await clickSel(cdp, '[data-action="error-dismiss"]');

  // 錯誤 banner 依本體的代碼翻譯（task 3.3；spec「進度被拒以代碼翻譯」「未知代碼退回原文」）：頁面內假 fetch 回帶 code 的 409。
  // 英文：有代碼用範本加參數（參數值原樣），字典沒有的代碼或沒有 code 的本體顯示原文 error；繁中一律顯示原文 error。
  {
    const LABEL = 'POST /api/projects/cockpit/tasks/be-1/advance';
    const prefixFor = (status) => (en ? `Action failed (HTTP ${status}, ${LABEL}): ` : `操作失敗（HTTP ${status}，${LABEL}）：`);
    const BANNER_CASES = [
      ['already_last_stage', { error: '已是最後一個 Stage', code: 'already_last_stage' }, 'Already at the last stage', '已是最後一個 Stage'],
      ['參數原樣（含中文）', { error: 'project 不存在：專案甲', code: 'unknown_project', params: { id: '專案甲' } }, 'Unknown project: 專案甲', 'project 不存在：專案甲'],
      ['persist_failed（原文含路徑）', { error: '寫入狀態檔失敗（C:\\x\\s.json）：disk full', code: 'persist_failed', params: { detail: 'disk full' } },
        'Failed to write the state file: disk full', '寫入狀態檔失敗（C:\\x\\s.json）：disk full'],
      ['未知代碼', { error: '某個新的後端原因', code: 'no_such_code', params: { a: '1' } }, '某個新的後端原因', '某個新的後端原因'],
      ['沒有 code', { error: '沒有代碼的原因' }, '沒有代碼的原因', '沒有代碼的原因'],
      // task 3.3 fix round 1：其餘 UI 可能收到的代碼（來源檢查、405、agent 端點）英文不得出現繁中；第 5 項是狀態碼，第 6 項要求英文結果不含 CJK。
      ['forbidden_source（Host 不符）', { error: 'Host 不是本機位址與實際監聽埠', code: 'forbidden_source' }, 'Request origin not accepted. Open this from the local Cockpit page', 'Host 不是本機位址與實際監聽埠', 403, true],
      ['forbidden_source（Origin 不符）', { error: 'Origin 與 Host 不符', code: 'forbidden_source' }, 'Request origin not accepted. Open this from the local Cockpit page', 'Origin 與 Host 不符', 403, true],
      ['method_not_allowed（寫入端點）', { error: '這個端點不接受這個 method', code: 'method_not_allowed' }, 'This operation is not allowed', '這個端點不接受這個 method', 405, true],
      ['method_not_allowed（輸出端點）', { error: '這個端點只接受 GET', code: 'method_not_allowed' }, 'This operation is not allowed', '這個端點只接受 GET', 405, true],
      ['missing_pane_id', { error: '缺少 X-Herdr-Pane-Id 標頭（值為 pane 內的 HERDR_PANE_ID）', code: 'missing_pane_id' },
        'Missing X-Herdr-Pane-Id header (the value is HERDR_PANE_ID inside the pane)', '缺少 X-Herdr-Pane-Id 標頭（值為 pane 內的 HERDR_PANE_ID）', 400, true],
      ['pane_not_bound（有 task、pane 參數）', { error: 'task be-1 所屬的 workstream 沒有綁定到 pane w1:p2', code: 'pane_not_bound', params: { task: 'be-1', pane: 'w1:p2' } },
        "The pane is not bound to this task's workstream", 'task be-1 所屬的 workstream 沒有綁定到 pane w1:p2', 403, true],
      ['pane_not_bound（鎖內重驗，沒有 params）', { error: 'pane 已不再綁定到該 workstream', code: 'pane_not_bound' },
        "The pane is not bound to this task's workstream", 'pane 已不再綁定到該 workstream', 403, true],
      ['internal_error（沒有 params）', { error: '寫入任務異常結束：task 1 panicked', code: 'internal_error' }, 'The write task ended abnormally', '寫入任務異常結束：task 1 panicked', 500, true],
      ['runtime_not_connected（Rejection）', { error: 'runtime 未連線', code: 'runtime_not_connected' }, 'Runtime is not connected', 'runtime 未連線', 409, true],
    ];
    await cdp.eval("window.__origFetch = window.fetch; undefined");
    for (const [name, body, wantEn, wantZh, status = 409, noCjk = false] of BANNER_CASES) {
      const prefix = prefixFor(status);
      await cdp.eval(`window.fetch = () => Promise.resolve(new Response(${J(JSON.stringify(body))}, { status: ${status}, headers: { 'Content-Type': 'application/json' } })); undefined`);
      pre(await clickSel(cdp, '[data-action="advance"][data-task="be-1"]'), `② [${L}] 按 be-1 的推進（假 409：${name}）`);
      await waitSel(cdp, '.error-banner');
      const got = await cdp.eval("document.querySelector('.error-banner .action-banner-text').textContent.trim()");
      const want = prefix + (en ? wantEn : wantZh);
      check(got === want, `② [${L}] 錯誤 banner（${name}）：${want}（實際 ${J(got)}）`);
      if (en && body.code === 'no_such_code') check(!got.includes('no_such_code'), `② [en] 錯誤 banner（${name}）：不顯示代碼本身`);
      if (en && noCjk) check(!CJK_RE.test(got), `② [en] 錯誤 banner（${name}）：英文介面不含任何 CJK 字元（實際 ${J(got)}）`);
      await clickSel(cdp, '[data-action="error-dismiss"]');
    }
    await cdp.eval('window.fetch = window.__origFetch; undefined');
  }

  // 通道斷線時頂列燈號 title 與底列 title（window.onChannel 不重畫）。
  await cdp.eval("window.onChannel('disconnected'); undefined");
  await snap('chanDown');
  await cdp.eval("window.onChannel('connected'); undefined");

  // 連線原因、protocol 警告、project 警告依代碼翻譯與退回（task 3.3；spec「WSL 未啟動的連線原因」「未知代碼退回原文」）：
  // 同一個同步 eval 內餵一份改過的投影再讀畫面（之後的推送會把它蓋回去）。英文：代碼有就用範本，字典沒有的代碼、欄位不存在、
  // 陣列比 warnings 短時，該筆顯示原文；任何情況都不顯示代碼本身。繁中一律顯示原文（沒有原文才用範本）。
  {
    const feedRead = (mod) => cdp.eval(`(() => {
      const s = structuredClone(window.cockpitLatestState());
      const wslConn = s.runtimes.find((r) => r.id === 'wsl').connection;
      const winConn = s.runtimes.find((r) => r.id === 'win').connection;
      const proj = s.projects.find((p) => p.id === 'cockpit');
      ${mod};
      window.onState(s);
      const tx = (sel) => Array.from(document.querySelectorAll(sel)).map((e) => e.textContent.trim());
      return { dd: tx('.connection-details dd'), proto: tx('.protocol-warning'), warn: tx('.project-warning'), ev: tx('.event-detail') };
    })()`);
    const ZH_RAW = 'WSL 發行版 Ubuntu-24.04 未啟動';
    const MSG_STATE_CASES = [
      ['reason 代碼未知', "wslConn.reason_msg = { code: 'no_such_code', params: { distro: 'X' } }", 'dd', ZH_RAW, ZH_RAW],
      ['reason_msg 欄位不存在（舊投影）', 'delete wslConn.reason_msg', 'dd', ZH_RAW, ZH_RAW],
      ['reason 是 raw 代碼', "wslConn.reason = 'connection refused (os error 111)'; wslConn.reason_msg = { code: 'raw', params: { text: 'connection refused (os error 111)' } }",
        'dd', 'connection refused (os error 111)', 'connection refused (os error 111)'],
      ['reason 的 detail 參數原樣（含中文）', "wslConn.reason = 'WSL 探測失敗：沒有權限'; wslConn.reason_msg = { code: 'wsl_probe_failed', params: { detail: '沒有權限' } }",
        'dd', 'WSL probe failed: 沒有權限', 'WSL 探測失敗：沒有權限'],
      ['protocol 警告有代碼', "winConn.protocol_warning = 'HERDR protocol 23 不在已測範圍 20..=22'; winConn.protocol_warning_msg = { code: 'protocol_untested', params: { protocol: '23', tested: '20..=22' } }",
        'proto', 'HERDR protocol 23 is outside the tested range 20..=22', 'HERDR protocol 23 不在已測範圍 20..=22'],
      ['protocol 警告代碼未知', "winConn.protocol_warning = '某個新的警告'; winConn.protocol_warning_msg = { code: 'no_such_code', params: {} }", 'proto', '某個新的警告', '某個新的警告'],
      ['protocol_warning_msg 欄位不存在', "winConn.protocol_warning = '某個新的警告'; delete winConn.protocol_warning_msg", 'proto', '某個新的警告', '某個新的警告'],
    ];
    for (const [name, mod, field, wantEn, wantZh] of MSG_STATE_CASES) {
      const got = await feedRead(mod);
      const want = en ? wantEn : wantZh;
      check(got[field].includes(want), `② [${L}] ${name}：${field === 'dd' ? '連線原因' : 'protocol 警告'}顯示 ${J(want)}（實際 ${J(got[field])}）`);
      check(got.dd.concat(got.proto).every((x) => !/no_such_code|^wsl_|^protocol_untested$|^raw$/.test(x)), `② [${L}] ${name}：畫面不顯示代碼本身`);
    }
    // 最近事件的 drift 說明（Cockpit 自己產生的狀態漂移原因）依 detail_msg 翻譯；代碼未知或欄位不存在退回原文。
    // 每個新增的 drift／payload 代碼各一筆；英文介面翻譯後的說明不得含 CJK（detail 參數是 serde 英文原文）。
    const EVENT_CASES = [
      ['workspace 不存在', 'workspace wZ 不存在', { code: 'drift_workspace_not_found', params: { id: 'wZ' } }, 'Workspace wZ does not exist'],
      ['tab 不存在', 'tab wZ:t9 不存在', { code: 'drift_tab_not_found', params: { id: 'wZ:t9' } }, 'Tab wZ:t9 does not exist'],
      ['pane 不存在', 'pane wZ:p9 不存在', { code: 'drift_pane_not_found', params: { id: 'wZ:p9' } }, 'Pane wZ:p9 does not exist'],
      ['runtime 未登記', 'runtime ghost 未登記', { code: 'drift_runtime_not_registered', params: { id: 'ghost' } }, 'Runtime ghost is not registered'],
      ['payload 無法解析', 'pane_created payload 無法解析：missing field `pane_id`',
        { code: 'event_payload_unparsable', params: { event: 'pane_created', detail: 'missing field `pane_id`' } },
        'Could not parse the pane_created event payload: missing field `pane_id`'],
    ];
    for (const [name, text, msgObj, wantEn] of EVENT_CASES) {
      const mod = `s.recent_events.unshift({ at: '2026-09-15T02:00:00Z', runtime: 'win', kind: 'drift', detail: ${J(text)}, detail_msg: ${J(msgObj)} })`;
      const got = await feedRead(mod);
      const want = en ? wantEn : text;
      check(got.ev[0] === want, `② [${L}] 最近事件 drift（${name}）：顯示 ${J(want)}（實際 ${J(got.ev[0])}）`);
      if (en) check(!CJK_RE.test(got.ev[0]) && !/drift_|event_payload/.test(got.ev[0]), `② [${L}] 最近事件 drift（${name}）：英文說明沒有 CJK、不顯示代碼本身`);
    }
    {
      const unknown = await feedRead("s.recent_events.unshift({ at: '2026-09-15T02:00:00Z', runtime: 'win', kind: 'drift', detail: '某個新的漂移原因', detail_msg: { code: 'no_such_code', params: {} } })");
      check(unknown.ev[0] === '某個新的漂移原因', `② [${L}] 最近事件 drift：代碼未知退回原文（實際 ${J(unknown.ev[0])}）`);
      const absent = await feedRead("s.recent_events.unshift({ at: '2026-09-15T02:00:00Z', runtime: 'win', kind: 'drift', detail: '某個新的漂移原因' })");
      check(absent.ev[0] === '某個新的漂移原因', `② [${L}] 最近事件 drift：沒有 detail_msg 欄位顯示原文（實際 ${J(absent.ev[0])}）`);
    }
    // project 警告：第一筆有代碼、第二筆沒有對應的 msg（陣列比 warnings 短）、第三筆代碼未知。
    const warnMod = `proj.warnings = ['甲警告', '乙警告', '丙警告'];
      proj.warning_msgs = [{ code: 'task_stage_reset', params: { task: 't1', stage: 'Old', start: 'Spec' } }, null, { code: 'no_such_code', params: {} }]`;
    const w1 = await feedRead(warnMod);
    const w1Want = en
      ? ['Task t1: stage "Old" is no longer in the pipeline stages; reset to the start stage "Spec"', '乙警告', '丙警告']
      : ['甲警告', '乙警告', '丙警告'];
    check(sameList(w1.warn, w1Want), `② [${L}] project 警告：有代碼的翻譯、空的與未知代碼的退回原文（${J(w1Want)}；實際 ${J(w1.warn)}）`);
    const w2 = await feedRead("proj.warnings = ['甲警告', '乙警告']; proj.warning_msgs = [{ code: 'task_stage_reset', params: { task: 't1', stage: 'Old', start: 'Spec' } }]");
    check(w2.warn.length === 2 && w2.warn[1] === '乙警告', `② [${L}] project 警告：warning_msgs 比 warnings 短時，多出的那筆顯示原文（實際 ${J(w2.warn)}）`);
    const w3 = await feedRead("proj.warnings = ['甲警告']; delete proj.warning_msgs");
    check(sameList(w3.warn, ['甲警告']), `② [${L}] project 警告：沒有 warning_msgs 欄位（舊投影）顯示原文（實際 ${J(w3.warn)}）`);
    await sleep(400); // 讓推送把正常投影畫回來
  }

  // 空狀態：同一個同步 eval 內餵一份沒有 Project 的投影再讀（之後的推送會把它蓋回去）。
  {
    const pageExpr = "const s = structuredClone(window.cockpitLatestState()); s.projects = []; window.onState(s);";
    if (en) {
      const r = await scanEnglish(cdp, matchers, 'empty', { pageExpr, probeMap: PROBE_MAP });
      snaps.empty = r.got;
    } else {
      snaps.empty = await cdp.eval(`(() => { const probe = ${probeUi.toString()}; ${pageExpr} return probe(${J(PROBE_MAP)}); })()`);
    }
    expectSnapshot(L, snaps, 'empty');
  }
  await sleep(400); // 讓推送把正常投影畫回來

  // Live Output：長輸出（truncated）、請求失敗（fetch reject）、非 JSON 的 502。
  pre(await clickSel(cdp, '.pane-row[data-pane="wJ:p3"]'), `② [${L}] 點 wJ:p3 的 pane 列`);
  const shown = await cdp.waitFor("(() => { const n = document.querySelector('.output-truncated-notice'); return !!n && !n.hidden; })()", 8000);
  pre(shown, `② [${L}] wJ:p3 的截斷提示顯示出來`);
  await snap('truncated');
  await cdp.eval("window.__origFetch = window.fetch; window.fetch = () => Promise.reject(new TypeError('network down')); undefined");
  pre(await cdp.waitFor("(() => { const n = document.querySelector('.output-error-reason'); return !!n && !n.hidden; })()", 8000), `② [${L}] 請求失敗的原因顯示出來`);
  await snap('netError');
  await cdp.eval("window.fetch = () => Promise.resolve(new Response('oops', { status: 502 })); undefined");
  pre(await cdp.waitFor("(document.querySelector('.output-error-reason') || {}).textContent === 'HTTP 502'", 8000), `② [${L}] 非 JSON 的 502 顯示 HTTP 502`);
  await snap('http502');

  // Live Output 的失敗原因依代碼翻譯（task 3.3）：503 output_read_failed（detail 原樣代入，可能是繁中或英文原文）、504 read_timeout、
  // 未知代碼與沒有 code 的本體顯示原文。pane 正被選取、輪詢中，假 fetch 換掉之後下一輪輪詢就會顯示。
  {
    const LIVE_CASES = [
      ['output_read_failed（Failed，原文含前綴）', 503, { error: '讀取 pane 輸出失敗：socket closed', code: 'output_read_failed', params: { detail: 'socket closed' } },
        'Failed to read pane output: socket closed', '讀取 pane 輸出失敗：socket closed'],
      ['output_read_failed（Unavailable，detail 是繁中原文）', 503, { error: 'ui_preview 模擬斷線（還剩 2 次恢復前）', code: 'output_read_failed', params: { detail: 'ui_preview 模擬斷線（還剩 2 次恢復前）' } },
        'Failed to read pane output: ui_preview 模擬斷線（還剩 2 次恢復前）', 'ui_preview 模擬斷線（還剩 2 次恢復前）'],
      ['output_read_failed（detail 是英文原文）', 503, { error: 'connection refused', code: 'output_read_failed', params: { detail: 'connection refused' } },
        'Failed to read pane output: connection refused', 'connection refused'],
      ['read_timeout', 504, { error: '讀取逾時', code: 'read_timeout' }, 'Read timed out', '讀取逾時'],
      ['forbidden_source', 403, { error: 'Origin 與 Host 不符', code: 'forbidden_source' }, 'Request origin not accepted. Open this from the local Cockpit page', 'Origin 與 Host 不符', true],
      ['method_not_allowed', 405, { error: '這個端點只接受 GET', code: 'method_not_allowed' }, 'This operation is not allowed', '這個端點只接受 GET', true],
      ['未知代碼', 503, { error: '某個新的後端原因', code: 'no_such_code', params: { a: '1' } }, '某個新的後端原因', '某個新的後端原因'],
      ['沒有 code', 503, { error: '沒有代碼的原因' }, '沒有代碼的原因', '沒有代碼的原因'],
    ];
    for (const [name, status, body, wantEn, wantZh, noCjk = false] of LIVE_CASES) {
      const want = en ? wantEn : wantZh;
      await cdp.eval(`window.fetch = () => Promise.resolve(new Response(${J(JSON.stringify(body))}, { status: ${status}, headers: { 'Content-Type': 'application/json' } })); undefined`);
      const ok = await cdp.waitFor(`(document.querySelector('.output-error-reason') || {}).textContent === ${J(want)}`, 8000);
      const got = await cdp.eval("(document.querySelector('.output-error-reason') || {}).textContent");
      check(ok, `② [${L}] Live Output（${name}）：${want}（實際 ${J(got)}）`);
      if (en && body.code === 'no_such_code') check(!String(got).includes('no_such_code'), `② [en] Live Output（${name}）：不顯示代碼本身`);
      if (en && noCjk) check(!CJK_RE.test(String(got)), `② [en] Live Output（${name}）：英文介面不含任何 CJK 字元（實際 ${J(got)}）`);
    }
  }
  await cdp.eval('window.fetch = window.__origFetch; undefined');
  await clickSel(cdp, '.output-close');

  // pane 已不存在：回 404（把 fetch 換成固定回 404 的假回應，與 live-output-check 的「pane 已不存在」同一條路徑）。
  await cdp.eval("window.fetch = () => Promise.resolve(new Response('{\"error\":\"x\"}', { status: 404, headers: { 'Content-Type': 'application/json' } })); undefined");
  pre(await clickSel(cdp, '.pane-row[data-pane="wJ:p3"]'), `② [${L}] 點 wJ:p3（回 404）`);
  pre(await cdp.waitFor("(() => { const n = document.querySelector('.output-gone-notice'); return !!n && !n.hidden; })()", 8000), `② [${L}] 「pane 已不存在」顯示出來`);
  await snap('gone');
  // 前端逾時：請求永遠不回（假 fetch 不 settle、忽略 AbortSignal），6 秒後前端自己放棄。
  await cdp.eval("window.fetch = () => new Promise(() => {}); undefined");
  pre(await clickSel(cdp, '.pane-row[data-pane="wJ:p3"]'), `② [${L}] 再點 wJ:p3（請求卡住）`);
  pre(await cdp.waitFor("(() => { const n = document.querySelector('.output-error-reason'); return !!n && !n.hidden; })()", 12000), `② [${L}] 前端逾時的原因顯示出來`);
  await snap('timeout');
  await cdp.eval('window.fetch = window.__origFetch; undefined');
  await clickSel(cdp, '.output-close');
}

// ---------------------------------------------------------------------------
// ② files.js／viewers.js 的畫面（task 2.2）
// ---------------------------------------------------------------------------

// 頁面內的假 fetch：依 window.__rules（`{ re, h }`，h 回 Promise<Response>）攔截，沒命中的照常送出。
// 與 runUiScenario 的 window.__origFetch 分開命名，兩段互不干擾。
const FAKE_INSTALL = `(() => {
  if (!window.__fakeOrigFetch) {
    window.__fakeOrigFetch = window.fetch;
    window.fetch = (url, opts) => {
      const u = String(url);
      for (const r of (window.__rules || [])) if (r.re.test(u)) return r.h(u, opts || {});
      return window.__fakeOrigFetch(url, opts);
    };
  }
  window.__rules = [];
  return true;
})()`;
const FAKE_RESTORE = `(() => { if (window.__fakeOrigFetch) { window.fetch = window.__fakeOrigFetch; window.__fakeOrigFetch = null; } window.__rules = []; return true; })()`;
// 假回應的 handler 原始碼（字串，送進頁面求值）。
const H = {
  json: (status, objSrc) => `(u, o) => Promise.resolve(new Response(JSON.stringify(${objSrc}), { status: ${status}, headers: { 'Content-Type': 'application/json' } }))`,
  // 永不回應；呼叫端 abort 時才 reject（讓 files.js 的輪詢鏈可以靠切換分頁收掉）。
  hang: () => `(u, o) => new Promise((_, rej) => { if (o.signal) o.signal.addEventListener('abort', () => rej(new DOMException('aborted', 'AbortError'))); })`,
  text: (ct, bodySrc) => `(u, o) => Promise.resolve(new Response(${bodySrc}, { status: 200, headers: { 'Content-Type': ${J(ct)} } }))`,
  // 照常取回中繼資料 JSON，再用 patchSrc（可改 b）改寫。
  metaPatch: (patchSrc) => `async (u, o) => { const r = await window.__fakeOrigFetch(u, o); const b = await r.json(); ${patchSrc}; return new Response(JSON.stringify(b), { status: 200, headers: { 'Content-Type': 'application/json' } }); }`,
};
const setRules = (cdp, rules) =>
  cdp.eval(`(() => { window.__rules = [${rules.map(([re, h]) => `{ re: ${re}, h: ${h} }`).join(', ')}]; return true; })()`);

// 目前分頁的檔案面板內的選擇器。
const FP = (sel) => `#review .file-panel:not([hidden]) ${sel}`;

const FILES_PROBE = {
  filesEmpty: ['#files-panel .files-empty', null],
  treeAria: ['#files-panel .files-tree', 'aria-label'],
  refresh: ['#files-panel .files-refresh', null],
  treeStatus: ['#files-panel .files-status', null],
  treeNotes: ['#files-panel .files-tree .tree-note', null],
  closeAria: ['.review-tab-close', 'aria-label'],
  closeTitle: ['.review-tab-close', 'title'],
  tabTitle: ['.review-tab-main[data-path]', 'title'],
  time: [FP('.file-toolbar-time'), null],
  vscode: [FP('.file-vscode'), null],
  stale: [FP('.file-stale-label:not([hidden])'), null],
  status: [FP('.file-status:not([hidden])'), null],
  placeholder: [FP('.file-placeholder'), null],
  noteTitle: [FP('.viewer-note-title'), null],
  noteMeta: [FP('.viewer-note-meta'), null],
  noteSizeTitle: [FP('.viewer-note-size'), 'title'],
  mdAnchor: [FP('.md-body a.anchor'), 'aria-label'],
  mdImgAlt: [FP('.md-img-alt'), null],
  mdImgTitle: [FP('.md-img-alt'), 'title'],
  mdInertTitle: [FP('.md-body a:not([href])'), 'title'],
  htmlTitle: [FP('iframe.html-frame'), 'title'],
  pdfBar: [FP('.pdf-toolbar'), 'aria-label'],
  pdfTools: [FP('.pdf-tool'), null],
  pdfPageTitle: [FP('.pdf-page-status'), 'title'],
  pdfZoomTitle: [FP('.pdf-zoom-level'), 'title'],
  pdfPages: [FP('.pdf-page'), 'aria-label'],
};

// 根目錄查詢與列目錄的錯誤文案（資料夾說法）：[code, 英文, 繁中（改動前逐字）]。unknown 是不在表裡的 code。
const FILES_TREE_ERRORS = [
  ['forbidden_source', 'Request origin not accepted. Open this from the local Cockpit page', '請求來源不被接受，請從本機的 Cockpit 頁面開啟'],
  ['method_not_allowed', 'This operation is not allowed', '這個操作不被接受'],
  ['bad_request', "The name contains characters that can't be handled, so it can't be read", '名稱含有無法處理的字元，無法讀取'],
  ['runtime_unknown', 'This runtime is not in the configuration', '設定中沒有這個 runtime'],
  ['pane_unknown', 'This pane is no longer on screen', '這個 pane 已不在目前的畫面中'],
  ['no_root', "This pane has no browsable folder (no working directory reported, or the directory doesn't exist)", '這個 pane 沒有可瀏覽的資料夾（沒有回報工作目錄，或該目錄不存在）'],
  ['root_unavailable', "This folder can't be browsed right now (not in an allowed root)", '這個資料夾目前無法瀏覽（不在允許的根目錄中）'],
  ['path_outside_root', 'This folder points outside the root, so its contents are not shown', '這個資料夾指向根目錄以外，不顯示內容'],
  ['not_found', 'Folder no longer exists', '資料夾已不存在'],
  ['wrong_kind', 'This item is no longer a folder', '這個項目已不是資料夾'],
  ['too_large', 'Content too large to display', '內容太大，無法顯示'],
  ['not_markdown', 'Not a Markdown file', '不是 Markdown 檔案'],
  ['io_error', 'Error while reading', '讀取時發生錯誤'],
  ['network', "Can't reach the Cockpit service", '無法連線到 Cockpit 服務'],
  ['timeout', 'Read timed out (no response in 10 s)', '讀取逾時（超過 10 秒沒有回應）'],
  ['zzz_not_in_table', 'Read failed (unrecognized response)', '讀取失敗（無法辨識的回應）'],
];
// 檔案分頁的錯誤文案：四個檔案專用說法，其餘沿用資料夾說法（挑逾時、連線失敗、無法辨識各一個）。
const FILES_FILE_ERRORS = [
  ['not_found', 'File no longer exists', '檔案已不存在'],
  ['wrong_kind', 'This item is no longer a file', '這個項目已不是檔案'],
  ['path_outside_root', 'This file points outside the root, so its content is not shown', '這個檔案指向根目錄以外，不顯示內容'],
  ['root_unavailable', "This root folder has no panes right now, so it can't be read", '這個根目錄目前沒有任何 pane，無法讀取'],
  ['timeout', 'Read timed out (no response in 10 s)', '讀取逾時（超過 10 秒沒有回應）'],
  ['network', "Can't reach the Cockpit service", '無法連線到 Cockpit 服務'],
  ['zzz_not_in_table', 'Read failed (unrecognized response)', '讀取失敗（無法辨識的回應）'],
];

const RE_READ_AT = { en: /^Read at \d\d:\d\d:\d\d$/, zh: /^讀取於 \d\d:\d\d:\d\d$/ };
const FILES_EXPECT = [
  ['empty', 'filesEmpty', { en: ['Select a pane in the Factory Floor or the runtime list first'], zh: ['先在 Factory Floor 或 runtime 清單選一個 pane'] }, 'has'],
  ['loadingRoot', 'treeStatus', { en: ['Loading root folder…'], zh: ['正在讀取根目錄…'] }, 'eq'],
  ['tree', 'treeAria', { en: ['File tree'], zh: ['檔案樹'] }, 'eq'],
  ['tree', 'refresh', { en: ['Refresh'], zh: ['重新整理'] }, 'eq'],
  ['loadingDir', 'treeNotes', { en: ['Loading…'], zh: ['讀取中…'] }, 'eq'],
  ['emptyDir', 'treeNotes', { en: ['(empty folder)'], zh: ['（空資料夾）'] }, 'eq'],
  ['more1', 'treeNotes', { en: ['1 more item not shown'], zh: ['還有 1 項未顯示'] }, 'eq'],
  ['more5', 'treeNotes', { en: ['5 more items not shown'], zh: ['還有 5 項未顯示'] }, 'eq'],
  ['dirError', 'treeNotes', { en: ['Folder no longer exists'], zh: ['資料夾已不存在'] }, 'eq'],
  ['fileLoading', 'time', { en: ['Not read yet'], zh: ['尚未讀取'] }, 'eq'],
  ['fileLoading', 'status', { en: ['Loading…'], zh: ['正在讀取…'] }, 'eq'],
  ['fileLoading', 'closeAria', { en: ['Close a.md'], zh: ['關閉 a.md'] }, 'eq'],
  ['fileLoading', 'closeTitle', { en: ['Close'], zh: ['關閉'] }, 'eq'],
  ['fileLoading', 'tabTitle', { en: ['docs/a.md\nRoot folder: review-repo'], zh: ['docs/a.md\n根目錄：review-repo'] }, 'eq'],
  ['md', 'time', { en: [RE_READ_AT.en], zh: [RE_READ_AT.zh] }, 'match'],
  ['md', 'vscode', { en: ['Open in VS Code'], zh: ['在 VS Code 開啟'] }, 'eq'],
  ['md', 'mdAnchor', { en: ['Link to heading "連結與圖片"', 'Link to heading "表格"', 'Link to heading "任務清單"', 'Link to heading "其他 GFM 元素"'], zh: ['連到標題「連結與圖片」', '連到標題「表格」', '連到標題「任務清單」', '連到標題「其他 GFM 元素」'] }, 'has'],
  ['md', 'mdImgAlt', { en: ['logo'], zh: ['logo'] }, 'eq'],
  ['md', 'mdImgTitle', { en: ["This image wasn't loaded (only images inside the root folder referenced by relative path are loaded)"], zh: ['這張圖片沒有載入（只載入根目錄內以相對路徑引用的圖片）'] }, 'eq'],
  ['mdFake', 'mdInertTitle', { en: ["This link won't open (it points outside the root folder, or isn't an http/https URL)", "This link won't open (it points outside the root folder, or isn't an http/https URL)"], zh: ['這個連結不會開啟（指向根目錄以外，或不是 http／https 網址）', '這個連結不會開啟（指向根目錄以外，或不是 http／https 網址）'] }, 'eq'],
  ['mdFake', 'mdImgAlt', { en: ['Image'], zh: ['圖片'] }, 'eq'],
  ['html', 'htmlTitle', { en: ['Contents of page.html (scripts disabled)'], zh: ['page.html 的內容（腳本已停用）'] }, 'eq'],
  ['pdf', 'pdfBar', { en: ['PDF pages and zoom'], zh: ['PDF 頁面與縮放'] }, 'eq'],
  ['pdf', 'pdfTools', { en: ['Previous page', 'Next page', 'Zoom out', 'Zoom in', 'Fit width'], zh: ['上一頁', '下一頁', '縮小', '放大', '符合寬度'] }, 'eq'],
  ['pdf', 'pdfPageTitle', { en: ['Current page / total pages'], zh: ['目前頁／總頁數'] }, 'eq'],
  ['pdf', 'pdfZoomTitle', { en: ['Zoom level'], zh: ['縮放比例'] }, 'eq'],
  ['pdf', 'pdfPages', { en: ['Page 1', 'Page 2', 'Page 3'], zh: ['第 1 頁', '第 2 頁', '第 3 頁'] }, 'eq'],
  ['pdfBad', 'noteTitle', { en: ["Can't parse this PDF"], zh: ['PDF 無法解析'] }, 'eq'],
  ['unsupported', 'noteTitle', { en: ['Preview not supported'], zh: ['不支援預覽'] }, 'eq'],
  ['unsupported', 'noteMeta', { en: [/^File size: \d+ B$/], zh: [/^檔案大小：\d+ B$/] }, 'match'],
  ['unsupported', 'noteSizeTitle', { en: [/^\d+ bytes$/], zh: [/^\d+ 位元組$/] }, 'match'],
  ['image', 'noteTitle', { en: ['Preview not supported'], zh: ['不支援預覽'] }, 'eq'],
  ['image', 'noteMeta', { en: [/^File size: \d+(\.\d)? (B|KB)$/], zh: [/^檔案大小：\d+(\.\d)? (B|KB)$/] }, 'match'],
  ['tooLarge', 'noteTitle', { en: ['File too large to preview'], zh: ['檔案太大，無法預覽'] }, 'eq'],
  ['tooLarge', 'noteMeta', { en: ['File size: 2.9 MB'], zh: ['檔案大小：2.9 MB'] }, 'eq'],
  ['tooLarge', 'noteSizeTitle', { en: ['3000000 bytes'], zh: ['3000000 位元組'] }, 'eq'],
  ['tooLargeUnknown', 'noteTitle', { en: ['File too large to preview'], zh: ['檔案太大，無法預覽'] }, 'eq'],
  ['tooLargeUnknown', 'noteMeta', { en: ['File size: Unknown'], zh: ['檔案大小：未知'] }, 'eq'],
  ['placeholder', 'placeholder', { en: ['The "bogus" viewer isn\'t implemented yet'], zh: ['尚未實作「bogus」檢視器'] }, 'eq'],
  ['fileStale', 'stale', { en: ['Stale'], zh: ['過期'] }, 'eq'],
  ['fileStale', 'status', { en: ['File no longer exists'], zh: ['檔案已不存在'] }, 'eq'],
];

// 一種語言走一遍 files.js／viewers.js 的各個畫面（左欄檔案樹、檔案分頁、各種檢視器、各錯誤狀態）。
// 檔案樹與檢視器的各種邊界狀態（空資料夾、「還有 N 項未顯示」、各錯誤碼、檔案太大等）靠頁面內的假 fetch 提供：
// 真的 fixture 沒有這些情況，而這段驗的是介面文字，不是後端行為。
async function runFilesScenario(cdp, matchers, base, L) {
  const en = L === 'en';
  const snaps = {};
  const probe = (map) => cdp.fn(probeUi, map);
  const snap = async (name, opts = {}) => {
    if (en) {
      const r = await scanEnglish(cdp, matchers, `files／${name}`, { ...opts, probeMap: FILES_PROBE });
      snaps[name] = r.got;
    } else {
      snaps[name] = await probe(FILES_PROBE);
    }
    expectSnapshot(L, snaps, name, FILES_EXPECT);
  };
  const textOf = (sel) => cdp.eval(`(() => { const e = document.querySelector(${J(sel)}); return e ? e.textContent.trim() : null; })()`);
  const rowSel = (p) => `#files-panel .files-tree .tree-row[data-path=${J(p)}]`;
  const tabSel = (p) => `.review-tab-main[data-path=${J(p)}]`;
  const clickRow = (p) => clickSel(cdp, rowSel(p));
  const toggleDir = async (p) => { await clickRow(p); await sleep(150); };
  // 重新展開一個資料夾：收合再展開（展開時才重讀該層）。
  const reopen = async (p) => { await toggleDir(p); await toggleDir(p); };
  const waitRead = async (kind, ms = 15000) => {
    const ok = await cdp.waitFor(`(() => {
      const t = document.querySelector(${J(FP('.file-toolbar-time'))});
      return !!document.querySelector(${J(FP(`[data-viewer="${kind}"]`))}) && !!t && /^(Read at|讀取於) /.test(t.textContent);
    })()`, ms);
    pre(ok, `② [${L}] 檔案分頁讀好 ${kind}`);
    return ok;
  };
  // 開（或切到）一個已在樹上看得到的檔案列。
  const openRow = async (p) => {
    pre(await waitSel(cdp, rowSel(p), 5000), `② [${L}] 檔案樹有列 ${p}`);
    await clickRow(p);
  };
  const open = async () => {
    await setSaved(cdp, base, L);
    await cdp.eval("localStorage.removeItem('cockpit.fileTabs'); true"); // 上一個語言開過的分頁不要還原回來
    const ok = await loadPage(cdp, `${base}/`);
    pre(ok, `② [${L}] 儀表板載入（${base}）`);
    return ok && (await cdp.waitFor("!!document.querySelector('.runtime-lamp') && !!document.querySelector('.pane-row')", 10000));
  };

  if (!pre(await open(), `② [${L}] 首份投影畫完`)) return;
  pre(await cdp.eval(FAKE_INSTALL), `② [${L}] 裝好假 fetch`);

  // 左欄切到「檔案」：沒選 pane 的空狀態。
  pre(await clickSel(cdp, '[data-left-tab="files"]'), `② [${L}] 點左欄「檔案」`);
  await snap('empty');

  // 選 pane，根目錄查詢卡住：正在讀取根目錄。
  await setRules(cdp, [['/\\/panes\\/[^/]+\\/root$/', H.hang()]]);
  pre(await clickSel(cdp, '.pane-row[data-pane="wJ:p5"]'), `② [${L}] 點 wJ:p5 的 pane 列（根目錄查詢卡住）`);
  pre(await cdp.waitFor("(() => { const n = document.querySelector('#files-panel .files-status'); return !!n && !n.hidden; })()", 5000), `② [${L}] 「正在讀取根目錄」顯示出來`);
  await snap('loadingRoot');

  // 換選 wJ:p4：真的根目錄與檔案樹。
  await setRules(cdp, []);
  pre(await clickSel(cdp, '.pane-row[data-pane="wJ:p4"]'), `② [${L}] 點 wJ:p4 的 pane 列`);
  pre(await waitSel(cdp, rowSel('README.md'), 10000), `② [${L}] 檔案樹出現 README.md`);
  await snap('tree');

  // 資料夾 docs 的各種狀態：讀取中、空、還有 N 項未顯示、讀取失敗。
  const listDocs = '/\\/list\\/docs$/';
  const entriesOf = (omitted, skipped, n) =>
    `{ entries: [${Array.from({ length: n }, (_, i) => `{ name: 'f${i}.txt', kind: 'file', icon: 'file.svg' }`).join(', ')}], omitted: ${omitted}, skipped: ${skipped} }`;
  await setRules(cdp, [[listDocs, H.hang()]]);
  await toggleDir('docs');
  pre(await cdp.waitFor("document.querySelectorAll('#files-panel .files-tree .tree-note').length === 1", 5000), `② [${L}] docs 讀取中的提示列出現`);
  await snap('loadingDir');
  for (const [name, rule, wantNote] of [
    ['emptyDir', H.json(200, entriesOf(0, 0, 0)), null],
    ['more1', H.json(200, entriesOf(1, 0, 1)), null],
    ['more5', H.json(200, entriesOf(2, 3, 1)), null],
    ['dirError', H.json(404, "{ code: 'not_found' }"), null],
  ]) {
    await setRules(cdp, [[listDocs, rule]]);
    await reopen('docs');
    pre(await cdp.waitFor("(() => { const n = document.querySelectorAll('#files-panel .files-tree .tree-note'); return n.length === 1 && !/^(Loading|讀取中)/.test(n[0].textContent); })()", 5000), `② [${L}] docs 的 ${name} 提示列出現`);
    await snap(name);
  }
  await setRules(cdp, []);
  await toggleDir('docs'); // 收合

  // 根目錄查詢的各錯誤碼：點「重新整理」→ 狀態列顯示對應文案。
  const treeErrors = [];
  for (const [code, enText, zhText] of FILES_TREE_ERRORS) {
    await setRules(cdp, [['/\\/panes\\/[^/]+\\/root$/', H.json(code === 'zzz_not_in_table' ? 418 : 400, `{ code: ${J(code)} }`)]]);
    await clickSel(cdp, '#files-panel .files-refresh');
    const shown = await cdp.waitFor("(() => { const n = document.querySelector('#files-panel .files-status'); return !!n && !n.hidden; })()", 5000);
    const text = await textOf('#files-panel .files-status');
    treeErrors.push({ code, shown, text, want: en ? enText : zhText });
  }
  const badTree = treeErrors.filter((x) => !x.shown || x.text !== x.want);
  check(badTree.length === 0, `② [${L}] 根目錄查詢 ${treeErrors.length} 個錯誤碼的文案逐字正確（不符：${J(badTree.slice(0, 3))}）`);
  if (en) {
    const hit = treeErrors.filter((x) => zhHits(matchers, x.text || '').length > 0);
    check(hit.length === 0, `② [en] 根目錄查詢的錯誤文案沒有繁中字典字串（命中 ${J(hit.slice(0, 3))}）`);
  }
  await setRules(cdp, []);
  await clickSel(cdp, '#files-panel .files-refresh');
  pre(await waitSel(cdp, rowSel('README.md'), 10000), `② [${L}] 重新整理後檔案樹回來`);

  // 檔案分頁：docs/a.md，中繼資料卡住 → 「尚未讀取」「正在讀取…」。
  await toggleDir('docs');
  await setRules(cdp, [['/\\/meta\\/docs\\/a\\.md$/', H.hang()]]);
  await openRow('docs/a.md');
  pre(await cdp.waitFor(`!!document.querySelector(${J(tabSel('docs/a.md'))}) && document.querySelector(${J(tabSel('docs/a.md'))}).getAttribute('aria-selected') === 'true'`, 5000), `② [${L}] docs/a.md 分頁開出來並選定`);
  pre(await cdp.waitFor(`(() => { const n = document.querySelector(${J(FP('.file-status:not([hidden])'))}); return !!n; })()`, 5000), `② [${L}] 讀取中的狀態列顯示出來`);
  await snap('fileLoading');
  // 切到 Live Output 收掉卡住的輪詢，再回來重讀。
  await clickSel(cdp, '#review-tab-live');
  await setRules(cdp, []);
  await clickSel(cdp, tabSel('docs/a.md'));
  await waitRead('markdown');

  // Markdown（README）：連結錨點 aria-label、被擋的圖片、工具列。
  await openRow('README.md');
  await waitRead('markdown');
  await cdp.waitFor(`document.querySelectorAll(${J(FP('.md-body a.anchor'))}).length >= 4`, 5000);
  await snap('md');

  // Markdown 的另兩個提示：不會開啟的連結、沒有 alt 的被擋圖片（假的渲染結果，再用中繼資料改版本觸發重讀）。
  await setRules(cdp, [
    ['/\\/render\\/README\\.md$/', H.text('text/html; charset=utf-8', J('<p><a href="../../outside.md">out</a> <a href="mailto:a@b.c">mail</a> <img src="https://example.com/x.png"></p>'))],
    ['/\\/meta\\/README\\.md$/', H.metaPatch('b.size += 1')],
  ]);
  pre(await cdp.waitFor(`document.querySelectorAll(${J(FP('.md-body a:not([href])'))}).length === 2`, 8000), `② [${L}] 假的 Markdown 渲染出現（兩個不會開啟的連結）`);
  await snap('mdFake');
  await setRules(cdp, []);

  // 純文字（note.txt）：太大（中繼資料 size 超過上限）、讀到的實際內容太大（大小未知）、尚未實作的檢視器。
  await openRow('note.txt');
  await waitRead('text');
  await setRules(cdp, [['/\\/meta\\/note\\.txt$/', H.metaPatch('b.size = 3000000')]]);
  pre(await cdp.waitFor(`!!document.querySelector(${J(FP('.viewer-note-title'))})`, 8000), `② [${L}] 「檔案太大」說明出現（中繼資料 size）`);
  await snap('tooLarge');
  await setRules(cdp, [
    ['/\\/meta\\/note\\.txt$/', H.metaPatch('b.size = 5')],
    ['/\\/raw\\/note\\.txt$/', H.text('text/plain', "'a'.repeat(2 * 1024 * 1024 + 16)")],
  ]);
  pre(await cdp.waitFor(`(() => { const m = document.querySelector(${J(FP('.viewer-note-meta'))}); return !!m && /(Unknown|未知)/.test(m.textContent); })()`, 8000), `② [${L}] 「檔案太大」說明出現（實際內容，大小未知）`);
  await snap('tooLargeUnknown');
  await setRules(cdp, [['/\\/meta\\/note\\.txt$/', H.metaPatch("b.viewer = 'bogus'; b.size += 7")]]);
  pre(await cdp.waitFor(`!!document.querySelector(${J(FP('.file-placeholder'))})`, 8000), `② [${L}] 「尚未實作」占位文字出現`);
  await snap('placeholder');
  await setRules(cdp, []);
  await waitRead('text');

  // HTML、PDF（含工具列、頁碼）、二進位（不支援預覽）。
  await openRow('page.html');
  await waitRead('html');
  await snap('html');
  await openRow('report.pdf');
  await waitRead('pdf', 20000);
  pre(await cdp.waitFor(`document.querySelectorAll(${J(FP('.pdf-page'))}).length === 3`, 10000), `② [${L}] PDF 三頁都建好`);
  await snap('pdf');
  await openRow('bin.dat');
  await waitRead('unsupported');
  await snap('unsupported');
  await openRow('docs/pic.png'); // 圖片檔：沒有圖片檢視器，中繼資料分類成不支援預覽
  await waitRead('unsupported');
  await snap('image');

  // PDF 無法解析：原始內容換成亂碼，再用中繼資料改版本觸發重讀。
  await openRow('report.pdf');
  await setRules(cdp, [
    ['/\\/raw\\/report\\.pdf$/', H.text('application/pdf', J('this is not a pdf'))],
    ['/\\/meta\\/report\\.pdf$/', H.metaPatch('b.size += 1')],
  ]);
  pre(await cdp.waitFor(`(() => { const n = document.querySelector(${J(FP('.viewer-note-title'))}); return !!n; })()`, 15000), `② [${L}] 「PDF 無法解析」說明出現`);
  await snap('pdfBad');
  await setRules(cdp, []);

  // 檔案分頁的錯誤狀態：留著上一次的內容並標「過期」；逐一換錯誤碼，點樹上的列立即重試。
  await openRow('note.txt');
  await waitRead('text');
  const fileErrors = [];
  for (const [i, [code, enText, zhText]] of FILES_FILE_ERRORS.entries()) {
    await setRules(cdp, [['/\\/meta\\/note\\.txt$/', H.json(code === 'zzz_not_in_table' ? 418 : 404, `{ code: ${J(code)} }`)]]);
    if (i > 0) await clickRow('note.txt'); // 已是目前分頁且上次讀取失敗：再點一次立即重試
    const want = en ? enText : zhText;
    const shown = await cdp.waitFor(`(() => { const n = document.querySelector(${J(FP('.file-status:not([hidden])'))}); return !!n && n.textContent.trim() === ${J(want)}; })()`, 6000);
    fileErrors.push({ code, shown, text: await textOf(FP('.file-status')) });
    if (i === 0) await snap('fileStale');
  }
  const badFile = fileErrors.filter((x) => !x.shown);
  check(badFile.length === 0, `② [${L}] 檔案分頁 ${fileErrors.length} 個錯誤碼的文案逐字正確（不符：${J(badFile.slice(0, 3))}）`);
  if (en) {
    const hit = fileErrors.filter((x) => zhHits(matchers, x.text || '').length > 0);
    check(hit.length === 0, `② [en] 檔案分頁的錯誤文案沒有繁中字典字串（命中 ${J(hit.slice(0, 3))}）`);
  }

  // 關閉分頁：關閉鈕的 aria-label 與 title 已在 fileLoading 驗過；這裡收尾。
  await setRules(cdp, []);
  await cdp.eval(FAKE_RESTORE);
}

// ---------------------------------------------------------------------------
// ② git.js 的畫面（task 2.3）
// ---------------------------------------------------------------------------

// 目前分頁（diff／Git Graph／某版本）面板下的「直屬狀態列」：不含複製回饋（aria-live）與 commit 詳情裡的狀態列。
const PSTATUS = '#review .file-panel:not([hidden]) > .file-status:not([hidden]):not([aria-live])';
const SEL_TAB_NOW = '.review-tab-main[aria-selected="true"]';

const GIT_PROBE = {
  // 左欄「變更」面板
  chButtons: ['#changes-panel .changes-head-actions .action-button', null],
  chBranch: ['#changes-panel .files-head-names > .files-runtime:nth-of-type(3):not([hidden])', null],
  chStatus: ['#changes-panel .files-status:not([hidden])', null],
  chStale: ['#changes-panel .file-stale-label:not([hidden])', null],
  chListAria: ['#changes-panel .files-tree', 'aria-label'],
  chHeadings: ['#changes-panel .changes-group-title', null],
  chNotes: ['#changes-panel .files-tree .tree-note', null],
  // 目前分頁的分頁標籤、title 與關閉鈕
  tabLabel: [`${SEL_TAB_NOW} .review-tab-label`, null],
  tabTitle: [SEL_TAB_NOW, 'title'],
  tabCloseAria: [`.review-tab:has(> ${SEL_TAB_NOW}) .review-tab-close`, 'aria-label'],
  tabCloseTitle: [`.review-tab:has(> ${SEL_TAB_NOW}) .review-tab-close`, 'title'],
  // diff 分頁與某版本檔案分頁（共用的版本標示、過期標籤、狀態列）
  versions: [FP('.diff-toolbar-versions'), null],
  dfButtons: [FP('.diff-toolbar .action-button'), null],
  gaps: [FP('.diff-gap'), null],
  stale: [FP('.file-stale-label:not([hidden])'), null],
  status: [PSTATUS, null],
  rvButtons: [FP('.file-toolbar .action-button'), null],
  placeholder: [FP('.file-placeholder'), null],
  // Git Graph 分頁
  gFilterToggle: [FP('[data-action="graph-filter-toggle"]'), null],
  gFilterAria: [FP('.graph-filter-popover'), 'aria-label'],
  gFilterActions: [FP('.graph-filter-actions .action-button'), null],
  gFilterGroups: [FP('.graph-filter-group-title'), null],
  gSearchPh: [FP('.graph-search-input'), 'placeholder'],
  gSearchAria: [FP('.graph-search-input'), 'aria-label'],
  gSearchCount: [FP('.graph-search-count'), null],
  gPrev: [FP('[data-action="graph-search-prev"]'), null],
  gNext: [FP('[data-action="graph-search-next"]'), null],
  gRefresh: [FP('[data-action="graph-refresh"]'), null],
  gBanner: [FP('.action-banner-text'), null],
  gReload: [FP('[data-action="graph-reload"]'), null],
  gListAria: [FP('.graph-listbox'), 'aria-label'],
  gLoadMore: [FP('[data-action="graph-load-more"]'), null],
  gNotes: [FP('.graph-scroll .tree-note'), null],
  gNotesShown: [FP('.graph-scroll .tree-note:not([hidden])'), null],
  copyFb: [FP('> .file-status[aria-live]:not([hidden])'), null],
  // commit 詳情與比較
  dLabels: [FP('.commit-detail-row > .commit-detail-label'), null],
  dFilesLabel: [FP('.commit-detail-row:has(+ .commit-detail-files) > .commit-detail-label'), null],
  dCmpHeader: [FP('.commit-detail > .commit-detail-row:first-child > .commit-detail-label'), null],
  dCopyText: [FP('.commit-detail [data-focus-key^="copy:"]'), null],
  dCopyAria: [FP('.commit-detail [data-focus-key^="copy:"]'), 'aria-label'],
  dActions: [FP('.commit-detail-actions .action-button'), null],
  dActionNote: [FP('.commit-detail-actions .tree-note'), null],
  dNotes: [FP('.commit-detail .tree-note'), null],
  dStatus: [FP('.commit-detail > .file-status'), null],
  dViewBtns: [FP('.commit-detail-file-row .action-button'), null],
  dParentMissing: [FP('.commit-detail-row > span[style*="font-family"]'), null],
};

// git 狀態端點與根目錄查詢的錯誤文案：[code, 英文, 繁中（改動前逐字）]。
const GIT_STATUS_ERRORS = [
  ['not_git', 'This root folder is not a git repo', '這個根目錄不是 git repo'],
  ['bad_request', 'Malformed request', '請求格式不正確'],
  ['git_unavailable', 'No usable git found', '找不到可用的 git'],
  ['git_untrusted', 'git refused to read this repo (owner mismatch)', 'git 拒絕讀取這個 repo（擁有者不符）'],
  ['git_timeout', 'Timed out', '執行逾時'],
  ['git_failed', 'Error while running git', '執行 git 時發生錯誤'],
  ['network', "Can't reach the Cockpit service", '無法連線到 Cockpit 服務'],
  ['timeout', 'Read timed out (no response in 10 s)', '讀取逾時（超過 10 秒沒有回應）'],
  ['rev_unknown', 'Version not found', '找不到這個版本'],
  ['ref_unknown', 'Branch not found', '找不到這個分支'],
  ['not_found_in_rev', 'The file does not exist in this version', '檔案在這個版本不存在'],
  ['no_merge_base', 'No common ancestor', '兩者沒有共同祖先'],
  ['too_large', 'Diff too large, view it in VS Code', '差異過大，請在 VS Code 查看'],
  ['unmerged_path', 'This file is still in a merge conflict, so the index has no single version to compare', '這個檔案還在合併衝突中，暫存區沒有單一版本可比較'],
  ['zzz_not_in_table', 'Read failed (unrecognized response)', '讀取失敗（無法辨識的回應）'],
];
const GIT_ROOT_ERRORS = [
  ...FILES_TREE_ERRORS.filter((e) => ['forbidden_source', 'method_not_allowed', 'bad_request', 'runtime_unknown', 'pane_unknown', 'no_root', 'network', 'timeout'].includes(e[0])),
  ['root_unavailable', "This root folder can't be browsed right now (not in an allowed root)", '這個根目錄目前無法瀏覽（不在允許的根目錄中）'],
  ['path_outside_root', 'Read failed (unrecognized response)', '讀取失敗（無法辨識的回應）'], // git.js 的根目錄表沒有這個 code，落到無法辨識
  ['zzz_not_in_table', 'Read failed (unrecognized response)', '讀取失敗（無法辨識的回應）'],
];

const H7 = '[0-9a-f]{7}';
const RX = (src) => new RegExp(src);
const BOTH = (v) => ({ en: v, zh: v });
const GIT_EXPECT = [
  // --- 左欄「變更」面板 ---
  ['chReal', 'chButtons', { en: ['Git Graph', 'Refresh'], zh: ['Git Graph', '重新整理'] }, 'eq'],
  ['chReal', 'chListAria', { en: ['Change list'], zh: ['變更清單'] }, 'eq'],
  ['chReal', 'chHeadings', { en: ['Staged (1)', 'Changes (2)', 'Untracked (1)'], zh: ['已暫存（1）', '變更（2）', '未追蹤（1）'] }, 'eq'],
  ['chReal', 'chBranch', BOTH(['main']), 'eq'],
  ['chReal', 'chStale', BOTH([]), 'eq'],
  ['chConflict', 'chHeadings', { en: ['Merge conflicts (1)'], zh: ['合併衝突（1）'] }, 'eq'],
  ['chConflict', 'chBranch', BOTH(['branch-b']), 'eq'],
  ['chLoading', 'chStatus', { en: ['Loading changes…'], zh: ['正在讀取變更…'] }, 'eq'],
  ['chDetached', 'chBranch', { en: [RX(`^Detached HEAD ${H7}$`)], zh: [RX(`^分離 HEAD ${H7}$`)] }, 'match'],
  ['chTruncated', 'chNotes', { en: ['Too many changes, showing only the first part'], zh: ['變更過多，只列出前面一部分'] }, 'eq'],
  ['chClean', 'chStatus', { en: ['No uncommitted changes'], zh: ['沒有未 commit 的變更'] }, 'eq'],
  ['chStale', 'chStale', { en: ['Stale'], zh: ['過期'] }, 'eq'],
  ['chStale', 'chStatus', { en: [GIT_STATUS_ERRORS[0][1]], zh: [GIT_STATUS_ERRORS[0][2]] }, 'eq'],
  ['chLoadingRoot', 'chStatus', { en: ['Loading root folder…'], zh: ['正在讀取根目錄…'] }, 'eq'],
  ['chNotGit', 'chStatus', { en: ['This root folder is not a git repo'], zh: ['這個根目錄不是 git repo'] }, 'eq'],
  // --- diff 分頁 ---
  ['diffStaged', 'tabLabel', { en: [RX('^staged-change\\.txt · Staged$')], zh: [RX('^staged-change\\.txt · 已暫存$')] }, 'match'],
  ['diffStaged', 'versions', { en: [RX(`^${H7} → Staged$`)], zh: [RX(`^${H7} → 已暫存$`)] }, 'match'],
  ['diffStaged', 'tabTitle', { en: [RX(`^history/staged-change\\.txt\\n${H7} → Staged\\nRoot folder: review-repo$`)], zh: [RX(`^history/staged-change\\.txt\\n${H7} → 已暫存\\n根目錄：review-repo$`)] }, 'match'],
  ['diffStaged', 'tabCloseAria', { en: ['Close staged-change.txt · Staged'], zh: ['關閉 staged-change.txt · 已暫存'] }, 'eq'],
  ['diffStaged', 'tabCloseTitle', { en: ['Close'], zh: ['關閉'] }, 'eq'],
  ['diffStaged', 'dfButtons', { en: ['Open file', 'Open in VS Code', 'View left version', 'View right version'], zh: ['開啟檔案', '在 VS Code 開啟', '看左側版本', '看右側版本'] }, 'eq'],
  ['diffUnstaged', 'tabLabel', { en: ['unstaged-change.txt · Working tree'], zh: ['unstaged-change.txt · 工作區'] }, 'eq'],
  ['diffUnstaged', 'versions', { en: ['Staged → Working tree'], zh: ['已暫存 → 工作區'] }, 'eq'],
  ['diffUnstaged', 'tabTitle', { en: ['history/unstaged-change.txt\nStaged → Working tree\nRoot folder: review-repo'], zh: ['history/unstaged-change.txt\n已暫存 → 工作區\n根目錄：review-repo'] }, 'eq'],
  ['diffUntracked', 'versions', { en: ['(empty) → Working tree'], zh: ['（空） → 工作區'] }, 'eq'],
  ['diffStale', 'stale', { en: ['Stale'], zh: ['過期'] }, 'eq'],
  ['diffStale', 'status', { en: ['Diff too large, view it in VS Code'], zh: ['差異過大，請在 VS Code 查看'] }, 'eq'],
  ['diffBinary', 'status', { en: ['Binary file, diff not shown'], zh: ['二進位檔，不顯示差異'] }, 'eq'],
  ['diffMode', 'status', { en: ['Only file permissions changed'], zh: ['只有權限改變'] }, 'eq'],
  ['diffSub', 'status', { en: ['Submodule, diff not shown'], zh: ['子模組，不顯示差異'] }, 'eq'],
  ['diffSame', 'status', { en: ['Both sides are identical'], zh: ['兩側內容相同'] }, 'eq'],
  ['diffGap', 'gaps', { en: ['1 line omitted', '12 lines omitted'], zh: ['省略 1 行', '省略 12 行'] }, 'eq'],
  ['diffLoading', 'status', { en: ['Loading diff…'], zh: ['正在讀取差異…'] }, 'eq'],
  ['diffErrLarge', 'status', { en: ['Diff too large, view it in VS Code'], zh: ['差異過大，請在 VS Code 查看'] }, 'eq'],
  ['diffErrUnmerged', 'status', { en: [GIT_STATUS_ERRORS[13][1]], zh: [GIT_STATUS_ERRORS[13][2]] }, 'eq'],
  // --- Git Graph 分頁 ---
  ['graph', 'tabLabel', BOTH(['Git Graph · review-repo']), 'eq'],
  ['graph', 'tabTitle', { en: ['Git Graph · review-repo\nruntime: win'], zh: ['Git Graph · review-repo\nruntime：win'] }, 'eq'],
  ['graph', 'tabCloseAria', { en: ['Close Git Graph · review-repo'], zh: ['關閉 Git Graph · review-repo'] }, 'eq'],
  ['graph', 'gFilterToggle', { en: ['Branch filter'], zh: ['分支篩選'] }, 'eq'],
  ['graph', 'gFilterAria', { en: ['Branch filter'], zh: ['分支篩選'] }, 'eq'],
  ['graph', 'gFilterActions', { en: ['Select all', 'Clear', 'Cancel', 'Apply'], zh: ['全選', '清除', '取消', '套用'] }, 'eq'],
  ['graph', 'gSearchPh', { en: ['Search commits'], zh: ['搜尋 commit'] }, 'eq'],
  ['graph', 'gSearchAria', { en: ['Search commits'], zh: ['搜尋 commit'] }, 'eq'],
  ['graph', 'gPrev', { en: ['Previous'], zh: ['上一筆'] }, 'eq'],
  ['graph', 'gNext', { en: ['Next'], zh: ['下一筆'] }, 'eq'],
  ['graph', 'gRefresh', { en: ['Refresh'], zh: ['重新整理'] }, 'eq'],
  ['graph', 'gBanner', { en: ['Branches changed'], zh: ['分支已變更'] }, 'eq'],
  ['graph', 'gReload', { en: ['Reload'], zh: ['重新載入'] }, 'eq'],
  ['graph', 'gListAria', { en: ['Commit list'], zh: ['Commit 清單'] }, 'eq'],
  ['graph', 'gLoadMore', { en: ['Load more'], zh: ['載入更多'] }, 'eq'],
  ['graph', 'gNotes', {
    en: ['Reached the limit of 5000 commits. Use the branch filter to narrow the range.', 'No commits to show'],
    zh: ['已達上限 5000 筆，可用分支篩選縮小範圍', '沒有可顯示的 commit'],
  }, 'eq'],
  ['graph', 'gNotesShown', BOTH([]), 'eq'],
  ['graph', 'status', BOTH([]), 'eq'],
  ['graphFilter', 'gFilterGroups', { en: ['Local branches', 'Remote branches', 'tag'], zh: ['本地分支', '遠端分支', 'tag'] }, 'eq'],
  ['gSearch1', 'gSearchCount', { en: ['1 match'], zh: ['共 1 筆'] }, 'eq'],
  ['gSearch1Go', 'gSearchCount', { en: ['1 of 1 match'], zh: ['第 1／共 1 筆'] }, 'eq'],
  ['gSearchMany', 'gSearchCount', { en: [RX('^\\d+ matches$')], zh: [RX('^共 \\d+ 筆$')] }, 'match'],
  ['gSearchManyGo', 'gSearchCount', { en: [RX('^1 of \\d+ matches$')], zh: [RX('^第 1／共 \\d+ 筆$')] }, 'match'],
  ['gSearchNone', 'gSearchCount', { en: ['No matches'], zh: ['沒有符合的結果'] }, 'eq'],
  ['detHead', 'dLabels', { en: ['hash', 'Author', 'Time', 'parents', 'Refs pointing to it'], zh: ['hash', '作者', '時間', 'parents', '指向它的 ref'] }, 'has'],
  ['detHead', 'dFilesLabel', { en: [RX('^Changed files? \\(\\d+\\)$')], zh: [RX('^變更檔案（\\d+）$')] }, 'match'],
  ['detHead', 'dCopyText', { en: ['Copy', 'Copy'], zh: ['複製', '複製'] }, 'eq'],
  ['detHead', 'dCopyAria', { en: ['Copy full hash', 'Copy "main"'], zh: ['複製完整 hash', '複製「main」'] }, 'eq'],
  ['detHead', 'dActions', { en: ['Set as compare base'], zh: ['選為比較基準'] }, 'eq'],
  ['detHead', 'dNotes', { en: ['Compared with the first parent commit'], zh: ['與第一個父 commit 比較'] }, 'eq'],
  ['detHead', 'dViewBtns', { en: ['View this version'], zh: ['看此版本'] }, 'has'],
  ['copyOk', 'copyFb', { en: ['Copied'], zh: ['已複製'] }, 'eq'],
  ['copyFail', 'copyFb', { en: ["Couldn't copy"], zh: ['無法複製'] }, 'eq'],
  ['cmpBase', 'dActionNote', { en: ['Set as compare base. Click another commit to compare.'], zh: ['已選為比較基準，點選另一個 commit 開始比較'] }, 'eq'],
  ['cmpDirect', 'dCmpHeader', { en: [RX(`^Compare ${H7} ↔ ${H7}$`)], zh: [RX(`^比較 ${H7} ↔ ${H7}$`)] }, 'match'],
  ['cmpDirect', 'dActions', { en: ['Direct comparison', 'From fork point'], zh: ['直接比較', '自分岔點起'] }, 'eq'],
  ['cmpDirect', 'dFilesLabel', { en: [RX('^Changed files? \\(\\d+\\)$')], zh: [RX('^變更檔案（\\d+）$')] }, 'match'],
  ['cmpLoading', 'dStatus', { en: ['Loading changes…'], zh: ['正在讀取變更…'] }, 'eq'],
  ['cmpErr', 'dStatus', { en: ['No common ancestor'], zh: ['兩者沒有共同祖先'] }, 'eq'],
  ['cmpTrunc', 'dNotes', { en: ['Too many changes, showing only the first part'], zh: ['變更過多，只列出前面一部分'] }, 'eq'],
  ['detLoading', 'dStatus', { en: ['Loading commit details…'], zh: ['正在讀取 commit 詳情…'] }, 'eq'],
  ['detErr', 'dStatus', { en: ['Version not found'], zh: ['找不到這個版本'] }, 'eq'],
  ['detPatched', 'dFilesLabel', { en: ['Changed file (1)'], zh: ['變更檔案（1）'] }, 'eq'],
  ['detPatched', 'dNotes', { en: ['Compared with the first parent commit', 'Too many changes, showing only the first part'], zh: ['與第一個父 commit 比較', '變更過多，只列出前面一部分'] }, 'eq'],
  ['detPatched', 'dParentMissing', { en: ['fffffff (not loaded)'], zh: ['fffffff（不在已載入範圍）'] }, 'eq'],
  ['graphBanner', 'gBanner', { en: ['Branches changed'], zh: ['分支已變更'] }, 'eq'],
  ['graphBanner', 'gReload', { en: ['Reload'], zh: ['重新載入'] }, 'eq'],
  ['graphEmpty', 'gNotesShown', { en: ['No commits to show'], zh: ['沒有可顯示的 commit'] }, 'eq'],
  ['graphErr', 'status', { en: ['Error while running git'], zh: ['執行 git 時發生錯誤'] }, 'eq'],
  ['graphLoading', 'status', { en: ['Loading commits…'], zh: ['正在讀取 commit…'] }, 'eq'],
  ['graphCap', 'gNotesShown', {
    en: ['Reached the limit of 5000 commits. Use the branch filter to narrow the range.'],
    zh: ['已達上限 5000 筆，可用分支篩選縮小範圍'],
  }, 'eq'],
  // --- 某版本檔案分頁 ---
  ['revReal', 'tabLabel', BOTH([RX(`^README\\.md @ ${H7}$`)]), 'match'],
  ['revReal', 'tabTitle', { en: [RX(`^README\\.md\\nVersion: ${H7}\\nRoot folder: review-repo$`)], zh: [RX(`^README\\.md\\n版本：${H7}\\n根目錄：review-repo$`)] }, 'match'],
  ['revReal', 'tabCloseAria', { en: [RX(`^Close README\\.md @ ${H7}$`)], zh: [RX(`^關閉 README\\.md @ ${H7}$`)] }, 'match'],
  ['revReal', 'rvButtons', { en: ['Open current version'], zh: ['開啟目前版本'] }, 'eq'],
  ['revReal', 'versions', BOTH([RX(`^${H7}$`)]), 'match'],
  ['revIndex', 'tabLabel', { en: ['staged-change.txt @ Staging area'], zh: ['staged-change.txt @ 暫存區'] }, 'eq'],
  ['revIndex', 'versions', { en: ['Staging area'], zh: ['暫存區'] }, 'eq'],
  ['revIndex', 'tabTitle', { en: ['history/staged-change.txt\nVersion: Staging area\nRoot folder: review-repo'], zh: ['history/staged-change.txt\n版本：暫存區\n根目錄：review-repo'] }, 'eq'],
  ['revStale', 'stale', { en: ['Stale'], zh: ['過期'] }, 'eq'],
  ['revStale', 'status', { en: ['The file does not exist in this version'], zh: ['檔案在這個版本不存在'] }, 'eq'],
  ['revLoading', 'status', { en: ['Loading…'], zh: ['正在讀取…'] }, 'eq'],
  ['revPlaceholder', 'placeholder', { en: ['The "bogus" viewer isn\'t implemented yet'], zh: ['尚未實作「bogus」檢視器'] }, 'eq'],
  ['revError', 'status', { en: ['The file does not exist in this version'], zh: ['檔案在這個版本不存在'] }, 'eq'],
];

// diff 分頁的假回應：依 `path` 查 window.__diffFakes（兩側都是 commit，分頁不輪詢，建立時只讀一次）。
const DIFF_BODY = (extra) => ({ status: 200, body: { version: 'fake', binary: false, mode_only: false, submodule: false, rows: [], ...extra } });
const CTX_ROW = (n, text) => ({ kind: 'context', left: { line: n, text }, right: { line: n, text } });
const DIFF_FAKES = {
  'fake/binary.dat': DIFF_BODY({ binary: true }),
  'fake/mode.sh': DIFF_BODY({ mode_only: true }),
  'fake/sub': DIFF_BODY({ submodule: true }),
  'fake/same.txt': DIFF_BODY({}),
  'fake/gap.txt': DIFF_BODY({ rows: [CTX_ROW(1, 'a'), { kind: 'gap', lines: 1 }, CTX_ROW(3, 'b'), { kind: 'gap', lines: 12 }, CTX_ROW(16, 'c')] }),
  'fake/loading.txt': { hang: true },
  'fake/err-large.txt': { status: 413, body: { code: 'too_large' } },
  'fake/err-unmerged.txt': { status: 409, body: { code: 'unmerged_path' } },
};
const FAKE_DIFF_H = `(u, o) => {
  const m = window.__diffFakes[new URL(u, location.href).searchParams.get('path')];
  if (m.hang) return new Promise((_, rej) => { if (o.signal) o.signal.addEventListener('abort', () => rej(new DOMException('aborted', 'AbortError'))); });
  return Promise.resolve(new Response(JSON.stringify(m.body), { status: m.status, headers: { 'Content-Type': 'application/json' } }));
}`;
// 一般的「照常取回再改寫」：patchSrc 可改 b（H.metaPatch 的泛用用法）。
const PATCH = (patchSrc) => H.metaPatch(patchSrc);
// 5000 筆的假 log：照常取回一小批，複製一列成 5000 個不同 oid 的列（驗「已達上限」提示）。
const FAKE_LOG_CAP_H = `async (u, o) => {
  const r = await window.__fakeOrigFetch(u.replace(/offset=\\d+/, 'offset=0').replace(/limit=\\d+/, 'limit=2'), o);
  const b = await r.json();
  const t = b.rows[1];
  b.rows = Array.from({ length: 5000 }, (_, i) => ({ ...t, oid: i.toString(16).padStart(40, '0'), parents: [] }));
  b.has_more = false;
  return new Response(JSON.stringify(b), { status: 200, headers: { 'Content-Type': 'application/json' } });
}`;
const RE_STATUS = '/\\/api\\/git\\/[^/]+\\/[^/]+\\/status/';
const RE_ROOT = '/\\/panes\\/[^/]+\\/root$/';
const RE_LOG = '/\\/api\\/git\\/[^/]+\\/[^/]+\\/log\\?/';
const RE_REFS = '/\\/api\\/git\\/[^/]+\\/[^/]+\\/refs$/';

// 一種語言走一遍 git.js 的各個畫面（左欄變更面板、diff 分頁、Git Graph、某版本分頁、各錯誤狀態）。
async function runGitScenario(cdp, matchers, base, L) {
  const en = L === 'en';
  const snaps = {};
  const snap = async (name, opts = {}) => {
    if (en) {
      const r = await scanEnglish(cdp, matchers, `git／${name}`, { ...opts, probeMap: GIT_PROBE });
      snaps[name] = r.got;
    } else {
      snaps[name] = await cdp.fn(probeUi, GIT_PROBE);
    }
    expectSnapshot(L, snaps, name, GIT_EXPECT);
  };
  const rules = (list) => setRules(cdp, list);
  const vis = (sel, ms = 8000) => cdp.waitFor(`(() => { const n = document.querySelector(${J(sel)}); return !!n && !n.hidden; })()`, ms);
  const textIs = (sel, text, ms = 8000) => cdp.waitFor(`(() => { const n = document.querySelector(${J(sel)}); return !!n && !n.hidden && n.textContent.trim() === ${J(text)}; })()`, ms);
  const textOf = (sel) => cdp.eval(`(() => { const e = document.querySelector(${J(sel)}); return e ? e.textContent.trim() : null; })()`);
  const openTabJs = (kind, fields) => cdp.eval(`(window.cockpitFiles.openTab(${J(kind)}, ${J(fields)}), true)`);
  const REFRESH = '#changes-panel .files-refresh:not([data-action])';
  const rowSel = (p) => `#changes-panel .changes-row[title=${J(p)}]`;
  const graphRow = (oid) => FP(`.graph-row[data-oid="${oid}"]`);
  const GRAPH_ROWS_SEL = FP('.graph-row');
  const open = async () => {
    await setSaved(cdp, base, L);
    await cdp.eval("localStorage.removeItem('cockpit.fileTabs'); true"); // 上一個語言開過的分頁不要還原回來
    const ok = await loadPage(cdp, `${base}/`);
    pre(ok, `② [${L}] 儀表板載入（${base}）`);
    return ok && (await cdp.waitFor("!!document.querySelector('.runtime-lamp') && !!document.querySelector('.pane-row')", 10000));
  };

  if (!pre(await open(), `② [${L}] 首份投影畫完`)) return;
  pre(await cdp.eval(FAKE_INSTALL), `② [${L}] 裝好假 fetch`);
  pre(await cdp.eval(`window.__diffFakes = ${J(DIFF_FAKES)}; true`), `② [${L}] 備好 diff 假回應`);

  // 選 wJ:p4（review-repo）、切到「變更」。
  pre(await clickSel(cdp, '.pane-row[data-pane="wJ:p4"]'), `② [${L}] 點 wJ:p4 的 pane 列`);
  pre(await clickSel(cdp, '[data-left-tab="changes"]'), `② [${L}] 點左欄「變更」`);
  if (!pre(await waitSel(cdp, '#changes-panel .changes-row', 10000), `② [${L}] 變更清單出現`)) return;
  const G = await cdp.eval(`(async () => {
    const j = async (u) => (await window.fetch(u)).json();
    const root = await j('/api/runtimes/win/panes/wJ:p4/root');
    const id = root.root_id;
    const st = await j('/api/git/win/' + id + '/status');
    const cm = await j('/api/git/win/' + id + '/commit/' + st.branch.oid);
    const refs = await j('/api/git/win/' + id + '/refs');
    const log = await j('/api/git/win/' + id + '/log?offset=0&limit=12');
    const f = refs.refs.find((r) => r.short === 'feature/logging');
    return { rootId: id, headOid: st.branch.oid, parentOid: cm.compared_to, fOid: f ? f.oid : null, rows: log.rows.map((r) => r.oid) };
  })()`);
  if (!pre(G && /^[0-9a-f]{40}$/.test(G.headOid) && /^[0-9a-f]{40}$/.test(G.parentOid) && /^[0-9a-f]{40}$/.test(G.fOid) && G.rows.length >= 8, `② [${L}] 取到 HEAD／父 commit／feature/logging 與前幾列的 oid（${J(G && { ...G, rows: G.rows && G.rows.length })}）`)) return;
  const REPO_FIELDS = { runtime: 'win', rootId: G.rootId, rootName: 'review-repo' };
  await snap('chReal');

  // --- diff 分頁：真的資料（暫存區的、未暫存的、未追蹤的列各開一個）---
  const openRowDiff = async (p) => {
    pre(await clickSel(cdp, rowSel(p)), `② [${L}] 點變更清單的 ${p}`);
    pre(await waitSel(cdp, FP('.diff-grid'), 8000), `② [${L}] ${p} 的 diff 格線出現`);
  };
  await openRowDiff('history/staged-change.txt');
  await snap('diffStaged');
  await openRowDiff('history/unstaged-change.txt');
  await snap('diffUnstaged');
  await openRowDiff('history/untracked-file.md');
  await snap('diffUntracked');
  // 工作區側的分頁每 2 秒輪詢：讀取失敗時保留上一次的內容並標「過期」。
  await rules([['/\\/diff\\?/', H.json(413, "{ code: 'too_large' }")]]);
  pre(await vis(FP('.file-stale-label'), 8000), `② [${L}] diff 輪詢失敗後顯示「過期」`);
  await snap('diffStale');
  await rules([]);

  // --- diff 分頁：假回應（兩側都是 commit，不輪詢）---
  await rules([['/\\/diff\\?.*path=fake\\//', FAKE_DIFF_H]]);
  const openFakeDiff = (path) => openTabJs('diff', { ...REPO_FIELDS, from: G.parentOid, to: G.headOid, path, oldPath: null });
  for (const [name, path, ready] of [
    ['diffBinary', 'fake/binary.dat', () => vis(PSTATUS)],
    ['diffMode', 'fake/mode.sh', () => vis(PSTATUS)],
    ['diffSub', 'fake/sub', () => vis(PSTATUS)],
    ['diffSame', 'fake/same.txt', () => vis(PSTATUS)],
    ['diffGap', 'fake/gap.txt', () => waitSel(cdp, FP('.diff-gap'))],
    ['diffLoading', 'fake/loading.txt', () => vis(PSTATUS)],
    ['diffErrLarge', 'fake/err-large.txt', () => textIs(PSTATUS, GIT_STATUS_ERRORS[12][en ? 1 : 2])],
    ['diffErrUnmerged', 'fake/err-unmerged.txt', () => textIs(PSTATUS, GIT_STATUS_ERRORS[13][en ? 1 : 2])],
  ]) {
    await openFakeDiff(path);
    pre(await ready(), `② [${L}] ${path} 的畫面出現`);
    await snap(name);
  }
  await rules([]);

  // --- Git Graph 分頁 ---
  pre(await clickSel(cdp, '#changes-panel [data-action="open-git-graph"]'), `② [${L}] 點「Git Graph」`);
  pre(await cdp.waitFor(`document.querySelectorAll(${J(GRAPH_ROWS_SEL)}).length >= 200`, 10000), `② [${L}] Git Graph 第一批 200 列出現`);
  await snap('graph');
  // 分支篩選（三組標題）。
  pre(await clickSel(cdp, FP('[data-action="graph-filter-toggle"]')), `② [${L}] 點「分支篩選」`);
  pre(await vis(FP('.graph-filter-popover')), `② [${L}] 分支篩選 popover 打開`);
  await snap('graphFilter');
  await clickSel(cdp, FP('[data-action="graph-filter-cancel"]'));
  // 搜尋：一筆（hash 前綴）、多筆、沒有。
  const searchInput = FP('.graph-search-input');
  const typeSearch = (v) => cdp.eval(`(() => { const i = document.querySelector(${J(searchInput)}); i.value = ${J(v)}; i.dispatchEvent(new Event('input', { bubbles: true })); return true; })()`);
  const searchEnter = () => cdp.eval(`(() => { const i = document.querySelector(${J(searchInput)}); i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })); return true; })()`);
  await typeSearch(G.headOid.slice(0, 8));
  await snap('gSearch1');
  await searchEnter();
  await snap('gSearch1Go');
  await typeSearch('fixture');
  await snap('gSearchMany');
  await searchEnter();
  await snap('gSearchManyGo');
  await typeSearch('zzzqq-no-such');
  await snap('gSearchNone');
  await typeSearch('');
  // commit 詳情：HEAD（merge commit）。
  pre(await clickSel(cdp, graphRow(G.headOid)), `② [${L}] 點 HEAD 那一列`);
  pre(await waitSel(cdp, FP('.commit-detail-hash'), 8000), `② [${L}] HEAD 的詳情出現`);
  await snap('detHead');
  // 複製回饋：成功、失敗。
  const COPY_BTN = FP('.commit-detail [data-focus-key^="copy:"]');
  await cdp.eval("Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: () => Promise.resolve() } }); true");
  await clickSel(cdp, COPY_BTN);
  pre(await vis(FP('> .file-status[aria-live]')), `② [${L}] 複製成功的回饋出現`);
  await snap('copyOk');
  await cdp.eval("Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: () => Promise.reject(new Error('denied')) } }); true");
  await clickSel(cdp, COPY_BTN);
  pre(await textIs(FP('> .file-status[aria-live]'), en ? "Couldn't copy" : '無法複製'), `② [${L}] 複製失敗的回饋出現`);
  await snap('copyFail');
  // 比較基準與比較詳情。
  pre(await clickSel(cdp, FP('[data-focus-key="compare-base"]')), `② [${L}] 點「選為比較基準」`);
  pre(await vis(FP('.commit-detail-actions .tree-note')), `② [${L}] 「已選為比較基準」提示出現`);
  await snap('cmpBase');
  pre(await clickSel(cdp, graphRow(G.fOid)), `② [${L}] 點 feature/logging 的 tip 那一列`);
  pre(await waitSel(cdp, FP('.commit-detail-row:has(+ .commit-detail-files)'), 8000), `② [${L}] 比較詳情的檔案清單出現`);
  await snap('cmpDirect');
  await rules([['/\\/changes\\?/', H.hang()]]);
  await clickSel(cdp, FP('[data-focus-key="toggle:fork"]'));
  pre(await vis(FP('.commit-detail > .file-status')), `② [${L}] 比較讀取中的狀態出現`);
  await snap('cmpLoading');
  await rules([]);
  await clickSel(cdp, FP('[data-focus-key="toggle:direct"]'));
  pre(await waitSel(cdp, FP('.commit-detail-row:has(+ .commit-detail-files)'), 8000), `② [${L}] 切回直接比較後檔案清單出現`);
  await rules([['/\\/merge-base\\?/', H.json(404, "{ code: 'no_merge_base' }")]]);
  await clickSel(cdp, FP('[data-focus-key="toggle:fork"]'));
  pre(await textIs(FP('.commit-detail > .file-status'), en ? 'No common ancestor' : '兩者沒有共同祖先'), `② [${L}] 沒有共同祖先的錯誤出現`);
  await snap('cmpErr');
  await rules([['/\\/changes\\?/', PATCH('b.truncated = true')]]);
  await clickSel(cdp, FP('[data-focus-key="toggle:direct"]'));
  pre(await waitSel(cdp, FP('.commit-detail .tree-note'), 8000), `② [${L}] 比較清單被截斷的提示出現`);
  await snap('cmpTrunc');
  await rules([]);
  // 回到乾淨狀態（重新整理會重設 session 與比較基準），再看單一 commit 詳情的各種狀態。
  await clickSel(cdp, FP('[data-action="graph-refresh"]'));
  pre(await cdp.waitFor(`document.querySelectorAll(${J(GRAPH_ROWS_SEL)}).length >= 200`, 10000), `② [${L}] 重新整理後 200 列回來`);
  const [, , , oidLoading, oidErr, oidPatched] = G.rows;
  await rules([[`/\\/commit\\/${oidLoading}/`, H.hang()], [`/\\/commit\\/${oidErr}/`, H.json(404, "{ code: 'rev_unknown' }")],
    [`/\\/commit\\/${oidPatched}/`, PATCH("b.truncated = true; b.parents = b.parents.concat(['f'.repeat(40)]); b.files = [{ path: 'fake.txt', status: 'M', icon: 'file.svg', additions: 1, deletions: 0, old_path: null }]")]]);
  await clickSel(cdp, graphRow(oidLoading));
  pre(await vis(FP('.commit-detail > .file-status')), `② [${L}] 詳情讀取中的狀態出現`);
  await snap('detLoading');
  await clickSel(cdp, graphRow(oidErr));
  pre(await textIs(FP('.commit-detail > .file-status'), en ? 'Version not found' : '找不到這個版本'), `② [${L}] 詳情讀取失敗的狀態出現`);
  await snap('detErr');
  await clickSel(cdp, graphRow(oidPatched));
  pre(await waitSel(cdp, FP('.commit-detail-row:has(+ .commit-detail-files)'), 8000), `② [${L}] 改寫過的詳情出現`);
  await snap('detPatched');
  await rules([]);
  // 分支已變更提示（refs 輪詢發現 tips 變了）。
  await rules([[RE_REFS, PATCH("b.refs[0].oid = 'a'.repeat(40)")]]);
  pre(await vis(FP('.action-banner')), `② [${L}] 「分支已變更」提示出現`);
  await snap('graphBanner');
  await rules([]);
  // 清單狀態：空、讀取失敗、讀取中、已達上限。
  await rules([[RE_LOG, H.json(200, '{ tips: [], rows: [], has_more: false }')]]);
  await clickSel(cdp, FP('[data-action="graph-refresh"]'));
  pre(await waitSel(cdp, FP('.graph-scroll .tree-note:not([hidden])'), 8000), `② [${L}] 「沒有可顯示的 commit」出現`);
  await snap('graphEmpty');
  await rules([[RE_LOG, H.json(500, "{ code: 'git_failed' }")]]);
  await clickSel(cdp, FP('[data-action="graph-refresh"]'));
  pre(await textIs(PSTATUS, en ? 'Error while running git' : '執行 git 時發生錯誤'), `② [${L}] 清單讀取失敗的狀態出現`);
  await snap('graphErr');
  await rules([[RE_LOG, H.hang()]]);
  await clickSel(cdp, FP('[data-action="graph-refresh"]'));
  pre(await vis(PSTATUS), `② [${L}] 清單讀取中的狀態出現`);
  await snap('graphLoading');
  await rules([[RE_LOG, FAKE_LOG_CAP_H]]);
  await clickSel(cdp, FP('[data-action="graph-refresh"]'));
  pre(await waitSel(cdp, FP('.graph-scroll .tree-note:not([hidden])'), 15000), `② [${L}] 「已達上限」提示出現`);
  pre((await cdp.eval(`document.querySelectorAll(${J(GRAPH_ROWS_SEL)}).length`)) === 5000, `② [${L}] 假 log 的 5000 列都畫出來`);
  await snap('graphCap');
  await rules([]);
  // 關掉 Git Graph 分頁（5000 列的節點不留著拖慢後面的掃描）。
  await clickSel(cdp, '.review-tab:has(> .review-tab-main[data-graph-root]) .review-tab-close');
  pre(await cdp.waitFor("!document.querySelector('.review-tab-main[data-graph-root]')", 5000), `② [${L}] Git Graph 分頁關掉`);

  // --- 某版本檔案分頁 ---
  await openTabJs('rev', { ...REPO_FIELDS, rev: G.headOid, path: 'README.md' });
  pre(await waitSel(cdp, FP('[data-viewer="markdown"]'), 10000), `② [${L}] README.md 在 HEAD 的版本畫出來`);
  await snap('revReal');
  await openTabJs('rev', { ...REPO_FIELDS, rev: 'INDEX', path: 'history/staged-change.txt' });
  pre(await waitSel(cdp, FP('[data-viewer="text"]'), 10000), `② [${L}] staged-change.txt 在暫存區的版本畫出來`);
  await snap('revIndex');
  await rules([['/\\/meta\\/INDEX\\//', H.json(404, "{ code: 'not_found_in_rev' }")]]);
  pre(await vis(FP('.file-stale-label')), `② [${L}] 暫存區版本輪詢失敗後顯示「過期」`);
  await snap('revStale');
  await rules([]);
  await rules([[`/\\/meta\\/${G.headOid}\\/note\\.txt$/`, H.hang()]]);
  await openTabJs('rev', { ...REPO_FIELDS, rev: G.headOid, path: 'note.txt' });
  pre(await vis(PSTATUS), `② [${L}] 某版本分頁讀取中的狀態出現`);
  await snap('revLoading');
  await rules([[`/\\/meta\\/${G.headOid}\\/docs\\/a\\.md$/`, PATCH("b.viewer = 'bogus'")]]);
  await openTabJs('rev', { ...REPO_FIELDS, rev: G.headOid, path: 'docs/a.md' });
  pre(await waitSel(cdp, FP('.file-placeholder'), 8000), `② [${L}] 「尚未實作」占位文字出現`);
  await snap('revPlaceholder');
  await rules([[`/\\/meta\\/${G.headOid}\\/long\\.md$/`, H.json(404, "{ code: 'not_found_in_rev' }")]]);
  await openTabJs('rev', { ...REPO_FIELDS, rev: G.headOid, path: 'long.md' });
  pre(await vis(PSTATUS), `② [${L}] 某版本分頁讀取失敗的狀態出現`);
  await snap('revError');
  await rules([]);

  // --- 左欄「變更」面板：各種狀態 ---
  // 換選 wJ:p5（other-repo，進行中的 merge 有衝突）：先卡住 status，看「正在讀取變更」，再放行看衝突組。
  await rules([[RE_STATUS, H.hang()]]);
  pre(await clickSel(cdp, '.pane-row[data-pane="wJ:p5"]'), `② [${L}] 點 wJ:p5 的 pane 列（status 卡住）`);
  pre(await textIs('#changes-panel .files-status', en ? 'Loading changes…' : '正在讀取變更…'), `② [${L}] 「正在讀取變更」出現`);
  await snap('chLoading');
  await rules([]);
  await clickSel(cdp, REFRESH);
  pre(await waitSel(cdp, '#changes-panel .changes-group-title', 8000), `② [${L}] other-repo 的衝突組出現`);
  await snap('chConflict');
  // 換回 wJ:p4，依序：分離 HEAD、清單被截斷、沒有變更。
  pre(await clickSel(cdp, '.pane-row[data-pane="wJ:p4"]'), `② [${L}] 點 wJ:p4 的 pane 列`);
  pre(await waitSel(cdp, '#changes-panel .changes-row', 8000), `② [${L}] p4 的變更清單回來`);
  await rules([[RE_STATUS, PATCH('b.branch.head = null')]]);
  await clickSel(cdp, REFRESH);
  pre(await cdp.waitFor("(() => { const n = document.querySelector('#changes-panel .files-head-names > .files-runtime:nth-of-type(3)'); return !!n && !n.hidden && n.textContent !== 'main'; })()", 8000), `② [${L}] 分離 HEAD 顯示出來`);
  await snap('chDetached');
  await rules([[RE_STATUS, PATCH('b.truncated = true')]]);
  await clickSel(cdp, REFRESH);
  pre(await waitSel(cdp, '#changes-panel .files-tree .tree-note', 8000), `② [${L}] 清單被截斷的提示出現`);
  await snap('chTruncated');
  await rules([[RE_STATUS, PATCH('b.entries = []')]]);
  await clickSel(cdp, REFRESH);
  pre(await textIs('#changes-panel .files-status', en ? 'No uncommitted changes' : '沒有未 commit 的變更'), `② [${L}] 「沒有未 commit 的變更」出現`);
  await snap('chClean');
  // git 狀態端點的各錯誤碼：保留上一次的內容並標「過期」，狀態列顯示對應文案。
  const statusErrors = [];
  for (const [i, [code, enText, zhText]] of GIT_STATUS_ERRORS.entries()) {
    await rules([[RE_STATUS, H.json(code === 'zzz_not_in_table' ? 418 : 400, `{ code: ${J(code)} }`)]]);
    await clickSel(cdp, REFRESH);
    const want = en ? enText : zhText;
    const shown = await textIs('#changes-panel .files-status', want, 6000);
    statusErrors.push({ code, shown, text: await textOf('#changes-panel .files-status') });
    if (i === 0) await snap('chStale');
  }
  const badStatus = statusErrors.filter((x) => !x.shown);
  check(badStatus.length === 0, `② [${L}] git 狀態端點 ${statusErrors.length} 個錯誤碼的文案逐字正確（不符：${J(badStatus.slice(0, 3))}）`);
  if (en) {
    const hit = statusErrors.filter((x) => zhHits(matchers, x.text || '').length > 0);
    check(hit.length === 0, `② [en] git 狀態端點的錯誤文案沒有繁中字典字串（命中 ${J(hit.slice(0, 3))}）`);
  }
  // 根目錄查詢的各錯誤碼。
  const rootErrors = [];
  for (const [code, enText, zhText] of GIT_ROOT_ERRORS) {
    await rules([[RE_ROOT, H.json(code === 'zzz_not_in_table' ? 418 : 400, `{ code: ${J(code)} }`)]]);
    await clickSel(cdp, REFRESH);
    const want = en ? enText : zhText;
    const shown = await textIs('#changes-panel .files-status', want, 6000);
    rootErrors.push({ code, shown, text: await textOf('#changes-panel .files-status') });
  }
  const badRoot = rootErrors.filter((x) => !x.shown);
  check(badRoot.length === 0, `② [${L}] 變更面板根目錄查詢 ${rootErrors.length} 個錯誤碼的文案逐字正確（不符：${J(badRoot.slice(0, 3))}）`);
  if (en) {
    const hit = rootErrors.filter((x) => zhHits(matchers, x.text || '').length > 0);
    check(hit.length === 0, `② [en] 變更面板根目錄查詢的錯誤文案沒有繁中字典字串（命中 ${J(hit.slice(0, 3))}）`);
  }
  // 根目錄查詢卡住、不是 git repo。
  await rules([[RE_ROOT, H.hang()]]);
  await clickSel(cdp, REFRESH);
  pre(await vis('#changes-panel .files-status'), `② [${L}] 「正在讀取根目錄」出現`);
  await snap('chLoadingRoot');
  await rules([[RE_ROOT, PATCH('b.is_git = false')]]);
  await clickSel(cdp, REFRESH);
  pre(await textIs('#changes-panel .files-status', en ? 'This root folder is not a git repo' : '這個根目錄不是 git repo'), `② [${L}] 「不是 git repo」出現`);
  await snap('chNotGit');
  await rules([]);
  await cdp.eval(FAKE_RESTORE);
}

// ---------------------------------------------------------------------------
// ② notify.js 的畫面（task 2.4）：鈴鐺、設定面板、權限四種狀態、桌面通知的標題與內文
// ---------------------------------------------------------------------------

const NOTIFY_PROBE = {
  bellLabel: ['.notify-bell', 'aria-label'],
  bellTitle: ['.notify-bell', 'title'],
  panelTitle: ['#notify-panel .notify-panel-title', null],
  panelNote: ['#notify-panel .notify-panel-note', null],
  kindNames: ['#notify-panel .notify-kind-name', null],
  kindDescs: ['#notify-panel .notify-kind-desc', null],
  permText: ['#notify-panel .notify-permission-text', null],
  permBtn: ['#notify-panel .notify-permission button', null],
};

const NOTIFY_PERM_TEXT = {
  default: { en: 'Notification permission: not decided yet.', zh: '通知權限：尚未決定。' },
  granted: { en: 'Notification permission: allowed.', zh: '通知權限：已允許。' },
  denied: {
    en: "Notification permission: blocked. To receive notifications, open your browser's site settings and set notifications to Allow for this site.",
    zh: '通知權限：已封鎖。要收到通知，請到瀏覽器的網站設定把這個網站的通知改為允許。',
  },
  unsupported: { en: 'This browser does not support desktop notifications.', zh: '這個瀏覽器不支援桌面通知。' },
};
// 面板快照 → 權限狀態（nAllowed＝在 default 狀態按下「允許通知」之後）。
const NOTIFY_SNAPS = { nDefault: 'default', nGranted: 'granted', nDenied: 'denied', nUnsupported: 'unsupported', nAllowed: 'granted' };
const NOTIFY_KIND_NAMES = ['agent blocked', 'agent done', 'task failed', 'task completed'];
const NOTIFY_EXPECT = Object.entries(NOTIFY_SNAPS).flatMap(([snap, perm]) => [
  [snap, 'bellLabel', { en: ['Notification settings'], zh: ['通知設定'] }, 'eq'],
  [snap, 'bellTitle', { en: ['Notification settings'], zh: ['通知設定'] }, 'eq'],
  [snap, 'panelTitle', { en: ['Desktop notifications'], zh: ['桌面通知'] }, 'eq'],
  [snap, 'panelNote', {
    en: ['Notifications are not shown while the Cockpit window is in the foreground.'],
    zh: ['Cockpit 視窗在前景時不跳出通知。'],
  }, 'eq'],
  [snap, 'kindNames', { en: NOTIFY_KIND_NAMES, zh: NOTIFY_KIND_NAMES }, 'eq'],
  [snap, 'kindDescs', {
    en: [
      'The agent is blocked and waiting for your response',
      'The agent has stopped and is waiting for you to take a look (this does not mean the task has ended)',
      'The task was marked failed',
      'The task was marked completed',
    ],
    zh: ['agent 卡住，等你回應', 'agent 停下，等你來看（不代表 task 結束）', 'task 被標記為 failed', 'task 被標記為 completed'],
  }, 'eq'],
  [snap, 'permText', { en: [NOTIFY_PERM_TEXT[perm].en], zh: [NOTIFY_PERM_TEXT[perm].zh] }, 'eq'],
  [snap, 'permBtn', snap === 'nDefault' ? { en: ['Allow notifications'], zh: ['允許通知'] } : { en: [], zh: [] }, 'eq'],
]);

// 頁面內的 Notification 記錄器（同 notify-check.js 的做法）：記下每則通知的 title／body／tag／renotify；
// permission 可指定，requestPermission 把權限改成 granted。前景判斷用 document.hasFocus 覆寫成 false，讓 observe 會發通知。
const NOTIFY_STUB = (permission) => `(() => {
  window.__notes = [];
  function N(title, opts) { window.__notes.push({ title, body: opts && opts.body, tag: opts && opts.tag, renotify: opts && opts.renotify }); }
  N.permission = ${J(permission)};
  N.requestPermission = (cb) => { N.permission = 'granted'; if (cb) cb('granted'); return Promise.resolve('granted'); };
  window.Notification = N;
  document.hasFocus = () => false;
  return true;
})()`;

// 以 cockpitNotify.observe 餵「基準→改過的投影」，回傳記錄器收到的通知。pane／task 取自 ui_preview 的 fixture：
// wJ:p4／wJ:p5 閒置且沒有綁定，wJ:p1 綁了 Backend 與 Undeclared 兩個 workstream（名稱去重後兩個），be-1 是 running 的 task。
const NOTIFY_RUN = `(() => {
  const clone = () => structuredClone(window.cockpitLatestState());
  const base = clone();
  const pane = (s, id) => { for (const r of s.runtimes) for (const w of r.workspaces || []) for (const t of w.tabs || []) for (const p of t.panes || []) if (p.id === id) return p; return null; };
  const task = (s, pid, tid) => s.projects.find((p) => p.id === pid).tasks.find((x) => x.id === tid);
  const run = (mutate) => {
    const next = clone();
    mutate(next);
    window.cockpitNotify.observe(base);
    window.__notes.length = 0;
    window.cockpitNotify.observe(next);
    return window.__notes.map((n) => ({ title: n.title, body: n.body, tag: n.tag, renotify: n.renotify }));
  };
  return {
    blocked: run((s) => { pane(s, 'wJ:p4').agent_status = 'blocked'; }),
    done: run((s) => { pane(s, 'wJ:p1').agent_status = 'done'; }),
    failed: run((s) => { task(s, 'cockpit', 'be-1').status = 'failed'; }),
    completed: run((s) => { task(s, 'cockpit', 'be-1').status = 'completed'; }),
    merged: run((s) => {
      pane(s, 'wJ:p1').agent_status = 'done';
      pane(s, 'wJ:p4').agent_status = 'blocked';
      pane(s, 'wJ:p5').agent_status = 'blocked';
      task(s, 'cockpit', 'be-1').status = 'failed';
    }),
  };
})()`;

// 每條：[名稱, { title, body, tag }]；title／body 各給 { en, zh }。task 標題「投影擴充」是使用者資料，兩種語言同文。
const NOTIFY_NOTE_EXPECT = {
  blocked: [{
    title: { en: 'agent blocked', zh: 'agent 卡住' }, body: { en: 'win / wJ:p4', zh: 'win / wJ:p4' }, tag: 'cockpit:blocked:win/wJ:p4',
  }],
  done: [{
    title: { en: 'agent stopped, take a look', zh: 'agent 停下等你看' },
    body: { en: 'win / wJ:p1 (Backend, Undeclared)', zh: 'win / wJ:p1（Backend、Undeclared）' }, tag: 'cockpit:done:win/wJ:p1',
  }],
  failed: [{
    title: { en: 'task failed', zh: 'task failed' }, body: { en: 'AI Cockpit: 投影擴充', zh: 'AI Cockpit：投影擴充' }, tag: 'cockpit:failed:cockpit/be-1',
  }],
  completed: [{
    title: { en: 'task completed', zh: 'task completed' }, body: { en: 'AI Cockpit: 投影擴充', zh: 'AI Cockpit：投影擴充' }, tag: 'cockpit:completed:cockpit/be-1',
  }],
  merged: [{
    title: { en: 'Cockpit: 4 items need attention', zh: 'Cockpit：4 件事需要注意' },
    body: {
      en: 'agent stopped, take a look · win / wJ:p1 (Backend, Undeclared)\nagent blocked · win / wJ:p4\nagent blocked · win / wJ:p5\n…',
      zh: 'agent 停下等你看 · win / wJ:p1（Backend、Undeclared）\nagent 卡住 · win / wJ:p4\nagent 卡住 · win / wJ:p5\n…',
    },
    tag: 'cockpit:summary',
  }],
};

async function runNotifyScenario(cdp, matchers, base, L) {
  const en = L === 'en';
  const snaps = {};
  const snap = async (name) => {
    if (en) {
      const r = await scanEnglish(cdp, matchers, name, { probeMap: NOTIFY_PROBE });
      snaps[name] = r.got;
    } else {
      snaps[name] = await cdp.fn(probeUi, NOTIFY_PROBE);
    }
    expectSnapshot(L, snaps, name, NOTIFY_EXPECT);
  };
  const bell = () => clickSel(cdp, '.notify-bell');
  const panelIs = (open) => cdp.waitFor(`(() => { const p = document.getElementById('notify-panel'); return !!p && p.hidden === ${!open}; })()`, 3000);

  await setSaved(cdp, base, L);
  await cdp.fn(() => { localStorage.removeItem('cockpit.notify.v1'); return true; });
  const ok = await loadPage(cdp, `${base}/`);
  pre(ok && (await cdp.waitFor("!!document.querySelector('.notify-bell') && !!document.querySelector('.ff-binding')", 10000)), `② [${L}] 儀表板載入（通知畫面）`);

  // 設定面板與權限四種狀態：每種各開一次面板。
  for (const [name, perm] of [['nDefault', 'default'], ['nGranted', 'granted'], ['nDenied', 'denied'], ['nUnsupported', null]]) {
    await cdp.eval(perm ? NOTIFY_STUB(perm) : 'window.Notification = undefined; true');
    pre(await bell(), `② [${L}] ${name}：按鈴鐺`);
    pre(await panelIs(true), `② [${L}] ${name}：設定面板開啟`);
    await snap(name);
    if (name === 'nDefault') {
      pre(await clickSel(cdp, '#notify-panel .notify-permission button'), `② [${L}] 按「允許通知」`);
      pre(await cdp.waitFor("!document.querySelector('#notify-panel .notify-permission button')", 3000), `② [${L}] 權限改為已允許、按鈕消失`);
      await snap('nAllowed');
    }
    await bell();
    pre(await panelIs(false), `② [${L}] ${name}：設定面板關閉`);
  }

  // 桌面通知：開啟 done、completed（預設關閉），再以記錄器收通知。
  await cdp.eval(NOTIFY_STUB('granted'));
  await bell();
  await panelIs(true);
  await cdp.eval(`(() => { for (const k of ['done', 'completed']) { const b = document.querySelector('#notify-panel input[data-notify-kind="' + k + '"]'); if (b && !b.checked) b.click(); } return true; })()`);
  const stored = await cdp.eval("localStorage.getItem('cockpit.notify.v1')");
  pre(stored === J({ blocked: true, done: true, failed: true, completed: true }), `② [${L}] 四種事件都已開啟（${stored}）`);
  await bell();
  const caps = await cdp.eval(NOTIFY_RUN);
  for (const [name, wantList] of Object.entries(NOTIFY_NOTE_EXPECT)) {
    const got = (caps && caps[name]) || [];
    const want = wantList.map((w) => ({ title: w.title[L], body: w.body[L], tag: w.tag, renotify: true }));
    check(J(got) === J(want), `② [${L}] 桌面通知「${name}」：標題／內文／tag／renotify 等於 ${J(want)}（實際 ${J(got)}）`);
    if (en) {
      const hits = got.flatMap((n) => [...zhHits(matchers, n.title), ...zhHits(matchers, n.body)]);
      check(hits.length === 0, `② [en] 桌面通知「${name}」沒有繁中字典字串（命中 ${J(hits.slice(0, 3))}）`);
    }
  }
  await cdp.fn(() => { localStorage.removeItem('cockpit.notify.v1'); return true; });
}

async function part2(cdp, port) {
  log('=== ② 英文介面沒有繁中字典字串（render.js／actions.js／output.js／files.js／viewers.js／git.js／notify.js）===');
  const notifyOnly = process.argv.includes('--notify-only'); // 開發用：只跑通知畫面那一段
  const gitOnly = notifyOnly || process.argv.includes('--git-only'); // 開發用：只跑 git 與通知畫面（--notify-only 連 git 也略過）
  const baseA = `http://127.0.0.1:${port}`;
  const srvA = startServer(port, { COCKPIT_PREVIEW_WRITE_RULES: '/api/projects/cockpit/tasks/be-1/fail=0:409' });
  let srvB = null;
  try {
    if (!pre(await waitUp(port), `ui_preview 在 10 秒內開始回應；stderr：${srvA.stderr().slice(0, 200)}`)) return;
    const loaded = loadI18nInSandbox({ saved: 'zh' });
    const matchers = buildZhMatchers(loaded.win.cockpitI18n.dictionaries.zh);
    pre(matchers.length > 40, `② 繁中比對器建好（${matchers.length} 個含 CJK 的字典值）`);
    // 比對器本身要有牙齒：繁中字串、帶資料的繁中句子、夾帶資料的長句都要抓得到；英文與使用者資料不誤報。
    check(zhHits(matchers, '作用中').length > 0 && zhHits(matchers, '警告 3').length > 0
      && zhHits(matchers, '歧義（2）').length > 0 && zhHits(matchers, '改綁模式：為 甲 / 乙 選一個 pane，按該列的「綁定到這裡」').length > 0
      && zhHits(matchers, '操作失敗（HTTP 500，GET /x）：壞了').length > 0,
    '② 比對器抓得到繁中字典字串（整段、帶資料的句子、夾帶資料的長句）');
    check(zhHits(matchers, 'Active').length === 0 && zhHits(matchers, '3 warnings').length === 0 && zhHits(matchers, '設計文件').length === 0,
      '② 比對器不誤報英文與不在字典裡的中文');

    if (!gitOnly) {
      await setEnv(cdp, { acceptLanguage: 'en-US', tz: NY });
      await runUiScenario(cdp, matchers, baseA, 'en');
      await setEnv(cdp, { acceptLanguage: 'en-US', tz: 'Asia/Taipei' });
      await runUiScenario(cdp, matchers, baseA, 'zh');
      check(cdp.exceptions.length === 0, `② 整段沒有未捕捉的頁面例外（${J(cdp.exceptions.slice(0, 3))}）`);
    }

    // files.js／viewers.js（task 2.2）、git.js（task 2.3）與 notify.js（task 2.4）：另起一個 ui_preview（推送間隔拉長，投影不再輪替），
    // 逐語言走檔案樹、檔案分頁，變更面板、diff、Git Graph、某版本分頁，以及通知鈴鐺、設定面板與桌面通知。
    const portB = pickPort(port + 1);
    srvB = startServer(portB, { COCKPIT_PREVIEW_PUSH_MS: '600000' });
    if (!pre(await waitUp(portB), `ui_preview（檔案、git 與通知畫面）在 10 秒內開始回應；stderr：${srvB.stderr().slice(0, 200)}`)) return;
    const baseB = `http://127.0.0.1:${portB}`;
    let exBefore = cdp.exceptions.length;
    if (!gitOnly) {
      await setEnv(cdp, { acceptLanguage: 'en-US', tz: NY });
      await runFilesScenario(cdp, matchers, baseB, 'en');
      await setEnv(cdp, { acceptLanguage: 'en-US', tz: 'Asia/Taipei' });
      await runFilesScenario(cdp, matchers, baseB, 'zh');
      check(cdp.exceptions.length === exBefore, `② 檔案畫面整段沒有未捕捉的頁面例外（${J(cdp.exceptions.slice(exBefore, exBefore + 3))}）`);
    }
    exBefore = cdp.exceptions.length;
    if (!notifyOnly) {
      await setEnv(cdp, { acceptLanguage: 'en-US', tz: NY });
      await runGitScenario(cdp, matchers, baseB, 'en');
      await setEnv(cdp, { acceptLanguage: 'en-US', tz: 'Asia/Taipei' });
      await runGitScenario(cdp, matchers, baseB, 'zh');
      check(cdp.exceptions.length === exBefore, `② git 畫面整段沒有未捕捉的頁面例外（${J(cdp.exceptions.slice(exBefore, exBefore + 3))}）`);
    }
    exBefore = cdp.exceptions.length;
    await setEnv(cdp, { acceptLanguage: 'en-US', tz: NY });
    await runNotifyScenario(cdp, matchers, baseB, 'en');
    await setEnv(cdp, { acceptLanguage: 'en-US', tz: 'Asia/Taipei' });
    await runNotifyScenario(cdp, matchers, baseB, 'zh');
    check(cdp.exceptions.length === exBefore, `② 通知畫面整段沒有未捕捉的頁面例外（${J(cdp.exceptions.slice(exBefore, exBefore + 3))}）`);
  } finally {
    try {
      await cdp.send('Emulation.setTimezoneOverride', { timezoneId: '' });
      await cdp.send('Page.navigate', { url: 'about:blank' });
    } catch {
      // 瀏覽器已關。
    }
    await sleep(300);
    killTree(srvA.proc, 'ui_preview（②）');
    if (srvB) killTree(srvB.proc, 'ui_preview（② 檔案與 git 畫面）');
  }
}

// ---------------------------------------------------------------------------
// ⑤ 英文介面 1536／1100／700 三種寬度截圖（交 frontend-design 審核）
// ---------------------------------------------------------------------------

// 截圖去識別化：作法同 docs/research/2026-10-02/output-color-check.js 的 maskExpression。ui_preview 的 fixture 把
// pane cwd 放在真實 %TEMP% 底下，畫面會出現 `C:\Users\<真實使用者名稱>\AppData\Local\Temp\…`。截圖前在頁面裝
// MutationObserver，把文字節點中 `Users\` 之後的路徑段與真實使用者名稱、主機名稱換成 `<user>`；背景重畫
// 一直重建節點，所以要持續替換。名稱在執行時由 os 取得，不寫進 repo。
function maskExpression() {
  const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const names = [os.userInfo().username, os.hostname()].filter(Boolean);
  const nameAlt = names.map(esc).join('|');
  return `(() => {
    const userRe = /(Users[\\\\/])[^\\\\/\\s]+/gi;
    const nameRe = new RegExp(${J(nameAlt)}, 'gi');
    const mask = (s) => s.replace(userRe, '$1<user>').replace(nameRe, '<user>');
    const ATTRS = ['title', 'aria-label', 'placeholder'];
    const sweep = (root) => {
      const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      for (let n = w.nextNode(); n; n = w.nextNode()) {
        const v = mask(n.nodeValue);
        if (v !== n.nodeValue) n.nodeValue = v;
      }
      // 屬性（title 等）畫面上看不到，但一併遮掉，讓截圖前的斷言能對整個頁面成立。
      for (const el of document.querySelectorAll('[title], [aria-label], [placeholder]')) {
        for (const a of ATTRS) {
          const v = el.getAttribute(a);
          if (v === null) continue;
          const m = mask(v);
          if (m !== v) el.setAttribute(a, m);
        }
      }
    };
    window.__maskObserver && window.__maskObserver.disconnect();
    window.__maskObserver = new MutationObserver(() => sweep(document.body));
    window.__maskObserver.observe(document.body, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ATTRS });
    sweep(document.body);
    return true;
  })()`;
}

// 截圖前確認頁面文字（textContent，含被 CSS 截斷的部分）與所有帶文字的屬性（title／aria-label／placeholder／value）
// 都不含真實使用者名稱或主機名稱，且文字裡沒有 `Users\` 之後接真實路徑段（只剩 `<user>`）。
async function assertMasked(cdp, label) {
  const names = [os.userInfo().username, os.hostname()].filter(Boolean);
  const text = await cdp.eval(`(() => {
    const parts = [document.body.textContent];
    for (const el of document.querySelectorAll('*')) for (const a of ['title', 'aria-label', 'placeholder', 'value']) { const v = el.getAttribute(a); if (v) parts.push(v); }
    return parts.join('\\n');
  })()`);
  const hit = names.filter((n) => text.toLowerCase().includes(n.toLowerCase()));
  const rawPath = /Users[\\/](?!<user>)[^\\/\s]+/i.test(text);
  pre(hit.length === 0 && !rawPath, `⑤ ${label}：截圖前頁面文字與屬性不含真實使用者名稱、主機名稱或未遮罩的 Users\\ 路徑（命中 ${hit.length} 個；未遮罩路徑 ${rawPath}）`);
  return hit.length === 0 && !rawPath;
}

const SHOT_SIZES = [[1536, 1024], [1100, 900], [700, 900]];
const SHOT_SCROLL_REVIEW = ['default', 'graph', 'file']; // 窄寬度時要捲到 #review 才看得到內容的畫面
const SHOT_SELECT_PANE = 'wJ:p4'; // 有真的檔案根目錄（README.md）與 git repo 的 pane

async function part5(cdp, port) {
  log('=== ⑤ 英文介面截圖（1536／1100／700）===');
  const srv = startServer(port, { COCKPIT_PREVIEW_PUSH_MS: '600000' }); // 投影不輪替，各張圖一致
  const wrote = [];
  try {
    if (!pre(await waitUp(port), `ui_preview（截圖）在 10 秒內開始回應；stderr：${srv.stderr().slice(0, 200)}`)) return;
    const base = `http://127.0.0.1:${port}`;
    const paneRow = `.pane-row[data-pane="${SHOT_SELECT_PANE}"]`;
    const scenes = [
      ['default', '預設儀表板（選了 pane、Live Output 顯示中）', async () => {
        pre(await waitSel(cdp, '#output .output-text', 10000), '⑤ default：Live Output 有內容區');
        return cdp.waitFor("(document.querySelector('#output .output-text') || { textContent: '' }).textContent.trim().length > 0", 10000);
      }],
      ['notify', '通知設定面板開啟', async () => {
        await cdp.eval(NOTIFY_STUB('default'));
        pre(await clickSel(cdp, '.notify-bell'), '⑤ notify：按鈴鐺');
        return cdp.waitFor("(() => { const p = document.getElementById('notify-panel'); return !!p && !p.hidden; })()", 5000);
      }],
      ['changes', '左欄「Changes」分頁', async () => {
        pre(await clickSel(cdp, '[data-left-tab="changes"]'), '⑤ changes：點左欄 Changes');
        return cdp.waitFor("document.querySelectorAll('#changes-panel .changes-group, #changes-panel [data-action=\"open-git-graph\"]').length > 0", 10000);
      }],
      ['graph', 'Git Graph 分頁', async () => {
        pre(await clickSel(cdp, '[data-left-tab="changes"]'), '⑤ graph：點左欄 Changes');
        pre(await waitSel(cdp, '#changes-panel [data-action="open-git-graph"]', 10000), '⑤ graph：Changes 面板有 Git Graph 按鈕');
        pre(await clickSel(cdp, '#changes-panel [data-action="open-git-graph"]'), '⑤ graph：點 Git Graph');
        return cdp.waitFor("document.querySelectorAll('#review .file-panel:not([hidden]) .graph-row, #review .file-panel:not([hidden]) [role=\"option\"]').length > 0", 15000);
      }],
      ['file', 'Markdown 檔案分頁', async () => {
        pre(await clickSel(cdp, '[data-left-tab="files"]'), '⑤ file：點左欄 Files');
        pre(await waitSel(cdp, '#files-panel .files-tree .tree-row[data-path="README.md"]', 10000), '⑤ file：檔案樹有 README.md');
        pre(await clickSel(cdp, '#files-panel .files-tree .tree-row[data-path="README.md"]'), '⑤ file：點 README.md');
        return waitSel(cdp, FP('[data-viewer="markdown"]'), 15000);
      }],
    ];
    // 開發用旗標：--shots=<畫面>-<寬度>[,…] 只重拍指定的幾張（例如 --only=5 --shots=file-1536）；沒給就全拍。
    const shotsArg = process.argv.slice(2).find((a) => /^--shots=/.test(a));
    const wanted = shotsArg ? shotsArg.slice(8).split(',') : null;
    for (const [scene, desc, act] of scenes) {
      for (const [w, h] of SHOT_SIZES) {
        const label = `${scene}-${w}`;
        if (wanted && !wanted.includes(label)) continue;
        await setEnv(cdp, { acceptLanguage: 'en-US', tz: NY });
        await setSaved(cdp, base, 'en');
        await cdp.fn(() => { localStorage.removeItem('cockpit.fileTabs'); localStorage.removeItem('cockpit.notify.v1'); return true; });
        await cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
        const ok = await loadPage(cdp, `${base}/`);
        pre(ok && (await cdp.waitFor("!!document.querySelector('.runtime-lamp') && !!document.querySelector('.pane-row')", 10000)), `⑤ [${label}] 儀表板載入`);
        if (!ok) continue;
        check((await cdp.eval('window.cockpitI18n.lang')) === 'en' && (await cdp.eval("document.documentElement.getAttribute('lang')")) === 'en', `⑤ [${label}] 英文介面（cockpitI18n.lang、<html lang> 都是 en）`);
        await cdp.eval(maskExpression());
        pre(await clickSel(cdp, paneRow), `⑤ [${label}] 點 ${SHOT_SELECT_PANE} 的 pane 列`);
        const ready = await act();
        pre(ready, `⑤ [${label}] ${desc}：畫面就緒`);
        await sleep(600);
        // 窄寬度時下半部（Live Output、Git Graph、檔案分頁）在第一屏之外：這幾個畫面把 #review 捲到視窗頂端再拍，其餘從頁首拍。
        if (SHOT_SCROLL_REVIEW.includes(scene) && w < 1536) await cdp.eval("document.getElementById('review').scrollIntoView({ block: 'start' }); true");
        else await cdp.eval('window.scrollTo(0, 0); true');
        await sleep(200);
        if (!ready || !(await assertMasked(cdp, label))) continue;
        const shot = await cdp.send('Page.captureScreenshot', { format: 'png' });
        const file = path.join(__dirname, `i18n-en-${scene}-${w}.png`);
        fs.writeFileSync(file, Buffer.from(shot.result.data, 'base64'));
        wrote.push(file);
        log(`wrote ${file}`);
      }
    }
    check(wrote.length === (wanted ? wanted.length : scenes.length * SHOT_SIZES.length), `⑤ 截圖都已產生（要拍 ${wanted ? wanted.length : scenes.length * SHOT_SIZES.length} 張，實際 ${wrote.length}）`);
    check(cdp.exceptions.length === 0, `⑤ 整段沒有未捕捉的頁面例外（${J(cdp.exceptions.slice(0, 3))}）`);
  } finally {
    try {
      await cdp.send('Emulation.clearDeviceMetricsOverride');
      await cdp.send('Emulation.setTimezoneOverride', { timezoneId: '' });
      await cdp.send('Page.navigate', { url: 'about:blank' });
    } catch {
      // 瀏覽器已關。
    }
    await sleep(300);
    killTree(srv.proc, 'ui_preview（⑤）');
  }
}

// 段代號 → { run, needsBrowser }。run 收到 (cdp, port, chrome)；不需要瀏覽器的段 cdp 為 null。
const PARTS = {
  1: { run: async () => part1(), needsBrowser: false },
  2: { run: (cdp, port) => part2(cdp, port), needsBrowser: true },
  3: { run: (cdp, port) => part3(cdp, port), needsBrowser: true },
  4: { run: (cdp, port, chrome) => part4(cdp, port, chrome), needsBrowser: true },
  5: { run: (cdp, port) => part5(cdp, port), needsBrowser: true },
};
const DEFAULT_PARTS = ['1', '2', '3', '4', '5'];

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

async function main() {
  const only = process.argv.slice(2).find((a) => /^--only=/.test(a));
  const parts = only ? only.slice(7).split(',') : DEFAULT_PARTS;
  const unknown = parts.filter((p) => !(p in PARTS));
  if (unknown.length) throw new Error(`不認得的段代號：${J(unknown)}（可用 ${Object.keys(PARTS).join(',')}）`);
  const needBrowser = parts.some((p) => PARTS[p].needsBrowser);
  let chrome = null;
  let port = 0;
  try {
    if (needBrowser) {
      if (!fs.existsSync(CHROME)) throw new Error(`找不到 Chrome：${CHROME}（可用環境變數 COCKPIT_CHROME 指定路徑）`);
      if (!fs.existsSync(UI_PREVIEW_EXE)) throw new Error(`找不到 ${UI_PREVIEW_EXE}，請先跑 cargo build -p cockpit --example ui_preview`);
      port = pickPort(7780);
      const cdpPort = pickPort(18830, [port]);
      chrome = await startChrome(cdpPort);
    }
    for (const p of parts) await PARTS[p].run(chrome && chrome.cdp, port, chrome);
  } catch (e) {
    pre(false, `中止：${e.stack || e.message}`);
  } finally {
    await stopChrome(chrome);
    if (needBrowser) {
      await sleep(300);
      pre(!isPortListening(port), `port ${port} 應該不再有 LISTENING 的行程`);
    }
  }
  console.log(`新行為斷言：${tally.spec.pass + tally.spec.fail} 條，PASS ${tally.spec.pass}、FAIL ${tally.spec.fail}`);
  console.log(`前置斷言：${tally.pre.pass + tally.pre.fail} 條，PASS ${tally.pre.pass}、FAIL ${tally.pre.fail}`);
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
