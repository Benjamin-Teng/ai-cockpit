// files-check.js：file-review 前端驗收腳本（file-review task 3.5）。headless Chrome＋CDP，啟動、收尾、
// 段落代號與「只跑指定段落」的寫法沿用 docs/research/2026-09-23/visual-check.js（startPreview／
// startChrome／killTree＋tasklist 收尾判準、「還握著 ChildProcess 且沒觀察到 exit 才終止」的行程所有權
// 模型、parseSegmentArg、打錯段名 exit 2），輸出請求計數與貼底判斷沿用
// docs/research/2026-09-19/live-output-check.js（NEAR_BOTTOM_PX＝6、`line N` 行號）。
//
// 對 `cockpit --example ui_preview` 逐條驗 openspec/changes/file-review/specs/ 中屬於前端的 scenario：
//   - file-review：「左欄檔案樹」「檔案分頁」「檔案檢視器」「自動更新」「分頁還原」「在 VS Code 開啟」
//     的全部 scenario（後端純 API 的 scenario 由 cockpit／cockpit-files 的 Rust 測試守，不在本檔）。
//   - live-output delta 新增的三個 scenario：「選定 pane 時切回 Live Output 分頁」「檔案分頁期間不請求
//     輸出」「切回時保持貼底」。
//   cockpit-dashboard delta 的「分頁很多不撐破頁面」「頻繁重畫不影響檔案分頁」「Markdown 檢視遵守色彩與
//   對比」三段在 visual-check.js（FT1／FT2／FT3），不在本檔。
//
// 這是「先寫測試」：前端（file-review 4.x）還沒做時，self/ 開頭的自我測試段必須通過，其餘每個 scenario
// 段都依預期失敗（RED），失敗訊息指出缺的是哪一條前端契約。某段在前端還沒做時竟然通過＝斷言太弱。
//
// 用法（repo 根；先 `cargo build -p cockpit --example ui_preview`）：
//   node docs/research/2026-09-27/files-check.js                          # 全部段落
//   node docs/research/2026-09-27/files-check.js "self/鷹架,file-review/中文 PDF"   # 只跑指定段落（逗號分隔）
//   node docs/research/2026-09-27/files-check.js live-output/             # 以 `<capability>/` 選該前綴的全部段落
//   node docs/research/2026-09-27/files-check.js --scratch=D:\tmp\shots   # 截圖存放目錄（預設 FILES_CHECK_SCRATCH 或 %TEMP%）
// 段落代號拼錯、空字串或只有逗號 → 印 `RESULT: FAIL (段落代號)`、exit 2，不啟動任何行程。
//
// 段落代號（每段對應 spec 的一個 scenario；capability 前綴＋spec 的 scenario 名稱）：
//   self/鷹架                                    腳本鷹架自我測試（見 segSelfScaffold 註解）
//   self/段落代號                                命令列段落代號驗證
//   file-review/左欄三個分頁                     左欄檔案樹（git-review task 4.2：三分頁 tablist）
//   file-review/切到檔案分頁                     左欄檔案樹
//   file-review/沒有選定 pane                    左欄檔案樹
//   file-review/展開狀態跨根目錄保留             左欄檔案樹
//   file-review/重畫不影響檔案樹                 左欄檔案樹
//   file-review/同一 pane 改 cwd 後檔案樹跟著換根  左欄檔案樹（最終修正波 F1：根目錄改變時讀取）
//   file-review/開檔新增分頁                     檔案分頁
//   file-review/重複開啟不新增                   檔案分頁
//   file-review/關閉目前分頁                     檔案分頁
//   file-review/切換 Project 不影響分頁          檔案分頁
//   file-review/md 相對連結在分頁區開啟          檔案檢視器
//   file-review/外部圖片不載入                   檔案檢視器
//   file-review/HTML 內的腳本不執行              檔案檢視器
//   file-review/沒有宣告編碼的 UTF-8 HTML        原始內容端點（change html-charset）
//   file-review/中文 PDF                         檔案檢視器
//   file-review/純文字不被解讀                   檔案檢視器
//   file-review/改檔後更新並保住捲動             自動更新
//   file-review/Live Output 分頁時不查詢         自動更新
//   file-review/檔案被刪掉後又出現               自動更新
//   file-review/重新整理後還原                   分頁還原
//   file-review/儲存內容損毀                     分頁還原
//   file-review/舊格式照常還原                   分頁還原（git-review task 4.1：v1 格式相容）
//   file-review/Windows 檔案                     在 VS Code 開啟
//   file-review/WSL 檔案                         在 VS Code 開啟
//   live-output/選定 pane 時切回 Live Output 分頁  選定一個 pane
//   live-output/檔案分頁期間不請求輸出           輪詢與顯示
//   live-output/切回時保持貼底                   輪詢與顯示
//
// ---------------------------------------------------------------------------
// 前端契約（file-review 4.x 照做；同一份整理在 files-check.md「前端契約」一節）
// ---------------------------------------------------------------------------
// 標 [spec] 的是 spec／design 明定；標 [約定] 的是本腳本約定、可與前端協調（改了要同步改本檔與 .md）。
//   C1 左欄分頁：`#files`（design D6）[spec] 內第一個 `role="tablist"`，其中兩個 `role="tab"`，可見文字
//      恰為「Project」「檔案」[spec 文字；role 為約定]；目前分頁 `aria-selected="true"` [約定]。
//   C2 檔案樹：`#files` 內 `role="tree"` [約定]；每一列一個 `role="treeitem"` [約定]，列是扁平排列
//      （以 `aria-level` 表示深度，不把子列巢狀放進 treeitem 內，讓點列中心一定點到該列）[約定]；
//      列的 `title` 恰為完整相對路徑（`/` 分隔）[spec]；資料夾列帶 `aria-expanded` [spec]；列可聚焦
//      （tabindex）[spec：鍵盤 Enter／Space]。樹頂端（tree 之外、tablist 之外）的可見文字含根目錄
//      `name` 與 runtime `id`，並有可見文字「重新整理」的 `<button>` [spec]。沒有選定 pane 時 `#files`
//      可見文字含「先在 Factory Floor 或 runtime 清單選一個 pane」[spec]。檔案樹的捲動容器是 tree
//      本身或其在 `#files` 內的祖先 [約定]。
//   C3 分頁列：`#review`（design D6）[spec] 內 `role="tablist"` [spec]，每個分頁一個 `role="tab"`
//      [約定]；第一個是 Live Output，可見文字含「Live Output」[約定]；檔案分頁的可見文字含檔名、
//      `title` 含完整相對路徑與根目錄名稱 [spec]，並帶 `data-path`＝相對路徑 [約定]；目前分頁
//      `aria-selected="true"` [約定]；每個 tab 以 `aria-controls` 指向自己的 `role="tabpanel"`，
//      非目前分頁的 tabpanel 設 `hidden`（design D6「切換分頁用 hidden 屬性」）[約定]。
//      Live Output 的 tabpanel 內含 `#output`（design D6）[spec]。切換分頁時先更新 aria-selected，再開始／停止
//      輪詢（請求分類以發出當下的 aria-selected 為準，見 pageHelpers）[約定]。
//   C4 關閉按鈕：`<button>`，`aria-label`（沒有時用可見文字）以「關閉」開頭 [約定]；放在該 tab 內，
//      或放在 tab 的包裝元素內（包裝元素不是 tablist 本身）[約定]。
//   C5 工具列：目前 tabpanel 內一個 `<a>`，可見文字恰為「在 VS Code 開啟」，`href`＝中繼資料的
//      `vscode_uri` 原值 [spec]。
//   C6 檢視器：目前 tabpanel 內一個帶 `data-viewer="markdown|text|html|pdf|unsupported"` 的元素包住
//      內容 [約定]；內容的捲動容器是它本身、它在 tabpanel 內的祖先，或 tabpanel 內第一個可捲動的
//      子孫 [約定]。PDF 每頁一個 `<canvas>`（2d context）[約定：design D7 的 canvas]；PDF 工具列在
//      tabpanel 可見文字中以「目前頁 / 總頁數」呈現（例如「1 / 3」）[spec]。HTML 檢視器是 tabpanel
//      內的 `<iframe>`，帶 `sandbox` 屬性 [spec]。
//   C7 過期標示：比照 Live Output，目前 tabpanel 的可見文字含「過期」，原因以可見文字呈現（例如
//      「檔案已不存在」）[「過期」文字為約定；原因文案為 spec]；恢復後兩者都不在可見文字中。
//   C8 分頁還原：寫在 `localStorage`，鍵名不限 [約定：「儲存內容損毀」段把當下所有 localStorage 值都
//      改成非法 JSON，不依賴鍵名]；讀到損毀值時 `console.warn` [spec：console 有警告]。
// 既有 DOM（不是本 change 新增）：pane 列 `.pane-row[data-runtime][data-pane]`（選定時加
// `.selected`）、Project 項目 `button[data-action="select-project"][data-project]`（目前的帶
// `aria-current="true"`）、Live Output 內容 `#output .output-text`、標題 `#output .output-title`。
//
// 預覽資料：ui_preview 啟動時把 cockpit/examples/fixtures/review-repo/ 複製到
// %TEMP%\cockpit-ui-preview-<pid>-<ns>\review-repo（stdout 印 `review-repo: <路徑>`），另建 other-repo；
// 假 pane `win/wJ:p4`（cwd＝review-repo/src）、`win/wJ:p5`（cwd＝other-repo）。spec scenario 裡的
// `w1:p1`／`w2:p1` 分別對應 `wJ:p4`／`wJ:p5`，`w1:p2` 對應 `wJ:p1`（ticker 輸出）。需要額外檔案
// （a.md／b.md／c.md、docs/a b.md、plan.md、nometa.html、many/）或改寫檔案的段落，一律只寫暫存副本，不碰 repo 內
// 的 fixture（self/鷹架 另外用雜湊確認 repo 內 fixture 沒被改）。
//
// 注意：本檔一次只開一個 ui_preview，也不要與 visual-check.js 等同時跑（計時斷言與收尾清查會互相干擾）。
// ui_preview 啟動時先綁定埠，綁定成功後才清理 %TEMP% 下前一次沒收掉的 `cockpit-ui-preview-<PID>-*`，且只刪 PID
// 已不存在的那些（見 cockpit/examples/ui_preview.rs 的 should_delete_stale_dir）。開跑前若偵測到有
// ui_preview.exe 在跑，或 127.0.0.1:7770 有人 LISTEN，就直接結束（exit 2），不動別人的行程。
'use strict';

const os = require('node:os');
const { spawn, spawnSync } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const crypto = require('node:crypto');

const REPO = path.resolve(__dirname, '..', '..', '..');
const UI_PREVIEW_EXE = path.join(REPO, 'target', 'debug', 'examples', 'ui_preview.exe');
const FIXTURE_SRC = path.join(REPO, 'cockpit', 'examples', 'fixtures', 'review-repo');
const CHROME =
  process.env.COCKPIT_CHROME || 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe';

// 同 visual-check.js：大多數段落不需要頻繁重畫，推送間隔拉長讓背景輪替在段落期間靜止。
const STABLE_PUSH_MS = '600000';
const CHROME_UDD_PREFIX = 'cockpit-chrome-filescheck-';
const PREVIEW_TEMP_PREFIX = 'cockpit-ui-preview-';
const RUNTIME = 'win';
const PANE_REVIEW = 'wJ:p4'; // cwd＝review-repo/src（spec 的 w1:p1）
const PANE_OTHER = 'wJ:p5'; // cwd＝other-repo（spec 的 w2:p1）
const PANE_TICKER = 'wJ:p1'; // 預設 ticker 輸出（spec 的 w1:p2）
const UI_TIMEOUT_MS = 5000;
const NEAR_BOTTOM_PX = 6; // 同 output.js／live-output-check.js 的貼底門檻
const NO_PANE_TEXT = '先在 Factory Floor 或 runtime 清單選一個 pane';
const GONE_FILE_TEXT = '檔案已不存在';
const STALE_TEXT = '過期';
const WSL_VSCODE_URI = 'vscode://vscode-remote/wsl+Ubuntu-24.04/home/u/repo/a.md:1';

// 中文 PDF 的像素判準（「3 頁都有畫出內容」「第 1 頁的中文標題可辨識」）：
// - 「非背景像素」＝與整張 canvas 出現最多的顏色（背景）任一通道（含 alpha）差超過 48 的像素。
// - PDF_PAGE_INK_MIN：整頁非背景像素比例下限。空白頁（全白或全透明）是 0；fixture 每頁至少有一段
//   14px 內文，self/鷹架 以 vendored pdf.js 畫出的實測值遠高於此（見該段輸出），取 0.2% 只為排除
//   「只畫了幾個點／一條線」的情況。
// - PDF_TITLE_BAND：第 1 頁標題所在的上方橫帶（頁高比例）。fixture（make-report-pdf.js）第 1 頁
//   padding 48px、h1 28px，標題約落在頁高 6%～10%，下一段內文約從 12% 開始；取 4%～11.5% 只含標題。
// - PDF_TITLE_INK_MIN：標題帶的非背景像素比例下限。空白頁為 0；self/鷹架 同時量「以 pdf.js 正確畫出的
//   第 1 頁」（正對照，實測值印在輸出）與「把標題帶塗白」（負對照，必須為 0），門檻取在兩者之間且
//   遠低於正對照，容許前端縮放不同造成的反鋸齒差異。像素比例分不出「字」與「方框（tofu）」，
//   所以該段另外存 viewport 截圖、印出路徑給人目視（spec「非方框」這半由目視確認）。
const PDF_PAGE_INK_MIN = 0.002;
const PDF_TITLE_BAND = { y0: 0.04, y1: 0.115 };
const PDF_TITLE_INK_MIN = 0.01;

// ---------------------------------------------------------------------------
// 命令列
// ---------------------------------------------------------------------------

const ARGV = process.argv.slice(2);
let SCRATCH = process.env.FILES_CHECK_SCRATCH || os.tmpdir();
const POSITIONAL = [];
for (const a of ARGV) {
  if (a.startsWith('--scratch=')) SCRATCH = a.slice('--scratch='.length);
  else POSITIONAL.push(a);
}
const SEGMENT_ARG = POSITIONAL[0];

// 代號以逗號分隔；每個代號必須完全等於某段的代號，或是以 `/` 結尾的 capability 前綴（選該前綴的全部
// 段落，至少要選中一段）。任何一個不合格就整體拒絕。
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

const failures = []; // { segment, label }
let CURRENT_SEGMENT = null;
function check(cond, label) {
  console.log(`${cond ? 'ok  ' : 'FAIL'} ${label}`);
  if (!cond) failures.push({ segment: CURRENT_SEGMENT, label });
  return !!cond;
}
// 前置條件：不成立就 FAIL 並結束這一段（後面的斷言建立在它之上，繼續跑只會產生一串連帶失敗）。
class SegmentAbort extends Error {}
function need(cond, label) {
  if (!check(cond, label)) throw new SegmentAbort(label);
}
const log = (s) => console.log(`[${new Date().toISOString()}] ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// 行程管理（同 visual-check.js：killTree／tasklist 判準、「還握著 ChildProcess 且沒觀察到 exit」才
// 送終止；一手來源見 visual-check.js finalSweep() 上方的長註解）
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

const SPAWNED = []; // { child, label, port }
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
// CDP（visual-check.js 的 CDP class 加上 flatten session 支援：sandbox iframe 若是 OOPIF，它的網路
// 事件只在子 session 出現，所以 Target.setAutoAttach 之後事件依 sessionId 分派）
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
  // 在頁面裡執行一個（不捕捉 Node 端變數的）函式；參數以 JSON 傳入。
  run(fn, ...args) {
    return this.eval(`(${fn.toString()})(${args.map((a) => JSON.stringify(a)).join(',')})`);
  }
  // 輪詢頁面函式直到回傳 truthy；回傳最後一次的值（逾時為 false），不自己 check()。
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
  // 點擊 fn(...args) 回傳的元素：捲進可視範圍、以中心點命中測試確認點得到它（或其子孫），再送真的
  // 滑鼠事件（同 visual-check.js click() 的做法）。
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
  // 沿用 visual-check.js 的 pressKey（file-review/左欄三個分頁 段：真的鍵盤事件，讓頁面的
  // keydown 監聽器（files.js handleTablistKeydown）判斷 event.key 時走到）。
  async pressKey(key, code, windowsVirtualKeyCode, text) {
    await this.send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, windowsVirtualKeyCode, text });
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode });
  }
}

async function startChrome(cdpPort, url, label, windowSize = '1536,1024') {
  const udd = fs.mkdtempSync(path.join(os.tmpdir(), `${CHROME_UDD_PREFIX}${label.replace(/[^A-Za-z0-9-]/g, '')}-`));
  const chrome = spawn(
    CHROME,
    [
      '--headless=new', '--lang=zh-TW',
      '--disable-gpu',
      '--no-first-run',
      `--remote-debugging-port=${cdpPort}`,
      '--remote-allow-origins=*',
      `--user-data-dir=${udd}`,
      `--window-size=${windowSize}`,
      url,
    ],
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
      // Windows 偶爾 EBUSY／EPERM（檔案還被剛結束的行程鎖著），稍後重試。
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
  const port = pickPort(7870);
  const info = { reviewRepo: null, otherRepo: null, outputRequests: [] };
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
      m = /^output-request (\S+) (\S+)$/.exec(line);
      if (m) info.outputRequests.push({ runtime: m[1], pane: m[2], at: Date.now() });
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
  const preview = { server, port, ...info, info, tempRoot: info.reviewRepo ? path.dirname(info.reviewRepo) : null };
  if (preview.tempRoot) PREVIEW_TEMP_ROOTS.add(preview.tempRoot);
  if (!(up && info.reviewRepo && info.otherRepo)) {
    check(false, `${label} 應該在 10 秒內開始回應並印出 review-repo／other-repo 路徑（up=${up}，review-repo=${info.reviewRepo}，other-repo=${info.otherRepo}）`);
    await stopPreview(preview, `${label}（啟動失敗收尾）`);
    throw new Error(`${label} 沒有起來`);
  }
  return preview;
}

// ui_preview 以 taskkill /F 結束時沒有機會自己清暫存目錄（task 3.4 report「暫存目錄生命週期」），
// 由這裡刪；只刪名稱符合 ui_preview 前綴、且確實含 review-repo／other-repo 的那一個目錄。
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

// ---------------------------------------------------------------------------
// 頁面端工具：以 Page.addScriptToEvaluateOnNewDocument 安裝成 window.__fc（重新整理後仍在）。
// 前端契約 C1–C7 的定位規則全部集中在這裡。
// ---------------------------------------------------------------------------

function pageHelpers() {
  if (window.__fc) return;
  window.__fcMessages = [];
  window.addEventListener('message', (e) => {
    let d;
    try {
      d = typeof e.data === 'string' ? e.data : JSON.stringify(e.data);
    } catch (_) {
      d = '[unserializable]';
    }
    window.__fcMessages.push(d);
  });
  // 頁面端請求記錄（fix round 1 起；fix round 2 依控制端裁決 R23 改寫）：只由本腳本注入（在任何前端腳本之前），
  // 不改產品。fetch 與 XMLHttpRequest（open／send）都包，每筆在**發出當下同步**記下當時 Live Output tab 的
  // aria-selected 與目前分頁身分（`at`），settle 當下再記一次（`endAt`）。「這個請求是在 Live Output 可見時還是
  // 檔案分頁期間發出的」直接由 `at` 決定，不再用 MutationObserver 回呼時間當邊界——回呼在屬性變更之後的微任務
  // 才執行，同一個同步區塊裡「設定 aria-selected 後立即 fetch」的請求會落在回呼之前而被算錯邊（Codex fix round 1
  // re-review high）。前提（前端契約 C3）：切換分頁時先更新 aria-selected，再開始／停止輪詢。
  // 開始與 settle 時間用 performance.now()；`wall` 為對應的 epoch 毫秒（performance.timeOrigin＋now），只做紀錄。
  const tabState = () => {
    const root = document.getElementById('review');
    const tl = root ? root.querySelector('[role="tablist"]') : null;
    const tabs = tl ? Array.from(tl.querySelectorAll('[role="tab"]')) : [];
    const live = tabs.find((t) => /Live Output/.test(t.textContent || ''));
    const sel = tabs.find((t) => t.getAttribute('aria-selected') === 'true');
    return {
      live: live ? live.getAttribute('aria-selected') : null,
      tab: sel ? (sel === live ? 'LIVE' : sel.getAttribute('data-path')) : null,
    };
  };
  const absUrl = (u) => {
    try {
      return new URL(String(u), location.href).href;
    } catch (_) {
      return String(u);
    }
  };
  let reqSeq = 0;
  window.__fcRequests = [];
  const newRecord = (via, url) => {
    const rec = { id: ++reqSeq, via, url: absUrl(url), start: performance.now(), wall: performance.timeOrigin + performance.now(), at: tabState(), end: null, endAt: null, outcome: null };
    window.__fcRequests.push(rec);
    return rec;
  };
  const settle = (rec, outcome) => {
    if (rec.end !== null) return;
    rec.end = performance.now();
    rec.endAt = tabState();
    rec.outcome = outcome;
  };
  const origFetch = window.fetch;
  window.fetch = function (input) {
    const rec = newRecord('fetch', typeof input === 'string' ? input : (input && input.url) || input);
    const p = origFetch.apply(window, arguments);
    p.then(
      () => settle(rec, 'resolved'),
      () => settle(rec, 'rejected')
    );
    return p;
  };
  const XO = XMLHttpRequest.prototype.open;
  const XS = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (method, url) {
    this.__fcUrl = url;
    return XO.apply(this, arguments);
  };
  XMLHttpRequest.prototype.send = function () {
    const rec = newRecord('xhr', this.__fcUrl);
    this.addEventListener('loadend', () => settle(rec, this.status ? 'resolved' : 'rejected'));
    return XS.apply(this, arguments);
  };
  // 切回 Live Output 的時間點（量「切回後立即」用）：capture 階段的 click 監聽器在任何前端 handler 之前、同一個
  // 事件派送當下同步記錄，點在 Live Output tab 上的才記。
  window.__fcLiveClicks = [];
  window.addEventListener(
    'click',
    (e) => {
      const tab = e.target && e.target.closest ? e.target.closest('#review [role="tab"]') : null;
      if (tab && /Live Output/.test(tab.textContent || '')) window.__fcLiveClicks.push(performance.now());
    },
    true
  );
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
  const reviewRoot = () => document.getElementById('review');
  const leftTablist = () => (filesRoot() ? filesRoot().querySelector('[role="tablist"]') : null);
  const leftTabs = () => (leftTablist() ? Array.from(leftTablist().querySelectorAll('[role="tab"]')) : []);
  const leftTab = (name) => leftTabs().find((t) => txt(t) === name) || null;
  const leftSelected = () => {
    const t = leftTabs().find((x) => x.getAttribute('aria-selected') === 'true');
    return t ? txt(t) : null;
  };
  const tree = () => (filesRoot() ? filesRoot().querySelector('[role="tree"]') : null);
  const rows = () => (tree() ? Array.from(tree().querySelectorAll('[role="treeitem"]')) : []);
  const row = (p) => rows().find((r) => r.getAttribute('title') === p) || null;
  const rowInfo = (r) => ({
    title: r.getAttribute('title'),
    text: txt(r),
    expanded: r.getAttribute('aria-expanded'),
    level: r.getAttribute('aria-level'),
    visible: visible(r),
  });
  // 樹頂端文字：#files 內、tree 與左欄 tablist 之外的可見文字。
  const filesHeaderText = () => {
    const f = filesRoot();
    if (!f) return '';
    const t = tree();
    const tl = leftTablist();
    const parts = [];
    const walker = document.createTreeWalker(f, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if ((t && t.contains(n)) || (tl && tl.contains(n))) continue;
      if (!n.parentElement || !visible(n.parentElement)) continue;
      const s = n.textContent.trim();
      if (s) parts.push(s);
    }
    return parts.join(' ');
  };
  const refreshButton = () => {
    const f = filesRoot();
    if (!f) return null;
    const t = tree();
    return Array.from(f.querySelectorAll('button')).find((b) => txt(b) === '重新整理' && !(t && t.contains(b))) || null;
  };
  const reviewTablist = () => (reviewRoot() ? reviewRoot().querySelector('[role="tablist"]') : null);
  const reviewTabs = () => (reviewTablist() ? Array.from(reviewTablist().querySelectorAll('[role="tab"]')) : []);
  const isLive = (t) => /Live Output/.test(txt(t));
  const liveTab = () => reviewTabs().find(isLive) || null;
  const fileTab = (p) => reviewTabs().find((t) => t.getAttribute('data-path') === p) || null;
  const tabInfo = (t) => ({
    label: txt(t),
    path: t.getAttribute('data-path'),
    title: t.getAttribute('title'),
    selected: t.getAttribute('aria-selected') === 'true',
    live: isLive(t),
  });
  const tabsInfo = () => reviewTabs().map(tabInfo);
  const selectedTab = () => reviewTabs().find((t) => t.getAttribute('aria-selected') === 'true') || null;
  const panelOf = (t) => {
    if (!t) return null;
    const id = t.getAttribute('aria-controls');
    const p = id ? document.getElementById(id) : null;
    return p && p.getAttribute('role') === 'tabpanel' ? p : null;
  };
  const currentPanel = () => panelOf(selectedTab());
  const panelText = () => txt(currentPanel());
  const isCloseButton = (b) => /^關閉/.test((b.getAttribute('aria-label') || txt(b) || '').trim());
  const closeButtonFor = (t) => {
    if (!t) return null;
    let b = Array.from(t.querySelectorAll('button')).find(isCloseButton);
    if (b) return b;
    const w = t.parentElement;
    if (w && w.getAttribute('role') !== 'tablist') b = Array.from(w.querySelectorAll('button')).find(isCloseButton);
    return b || null;
  };
  const vscodeLink = () => {
    const p = currentPanel();
    return p ? Array.from(p.querySelectorAll('a')).find((a) => txt(a) === '在 VS Code 開啟') || null : null;
  };
  const viewer = () => {
    const p = currentPanel();
    return p ? p.querySelector('[data-viewer]') : null;
  };
  const scrollable = (n) => {
    const cs = getComputedStyle(n);
    return (cs.overflowY === 'auto' || cs.overflowY === 'scroll') && n.scrollHeight > n.clientHeight + 1;
  };
  const scrollerWithin = (start, stop) => {
    for (let n = start; n; n = n.parentElement) {
      if (scrollable(n)) return n;
      if (n === stop) break;
    }
    const scope = stop || start;
    if (!scope) return null;
    for (const n of scope.querySelectorAll('*')) if (scrollable(n)) return n;
    return null;
  };
  const contentScroller = () => {
    const p = currentPanel();
    if (!p) return null;
    return scrollerWithin(viewer() || p, p);
  };
  const treeScroller = () => (tree() ? scrollerWithin(tree(), filesRoot()) : null);
  const contract = () => ({
    files: !!filesRoot(),
    leftTabs: leftTabs().map((t) => ({ text: txt(t), selected: t.getAttribute('aria-selected') })),
    tree: !!tree(),
    rows: rows().length,
    review: !!reviewRoot(),
    reviewTablist: !!reviewTablist(),
    tabs: tabsInfo(),
  });
  // 非背景像素比例（見 Node 端 PDF_* 常數的說明）：背景＝出現最多的顏色（每通道量化成 4 bit 統計），
  // 任一通道（含 alpha）與背景差超過 48 算一個非背景像素；band＝{y0,y1} 頁高比例，null＝整張。
  const INK_DIFF = 48;
  const inkStats = (canvas, band) => {
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    if (!ctx) return { error: 'canvas 沒有 2d context' };
    const w = canvas.width;
    const h = canvas.height;
    if (!w || !h) return { error: 'canvas 尺寸為 0' };
    const data = ctx.getImageData(0, 0, w, h).data;
    const hist = new Map();
    const step = Math.max(1, Math.floor((w * h) / 40000));
    for (let i = 0; i < w * h; i += step) {
      const o = i * 4;
      const key = ((data[o] >> 4) << 12) | ((data[o + 1] >> 4) << 8) | ((data[o + 2] >> 4) << 4) | (data[o + 3] >> 4);
      hist.set(key, (hist.get(key) || 0) + 1);
    }
    let bgKey = 0;
    let best = -1;
    for (const [k, c] of hist) {
      if (c > best) {
        best = c;
        bgKey = k;
      }
    }
    const bg = [((bgKey >> 12) & 15) * 16 + 8, ((bgKey >> 8) & 15) * 16 + 8, ((bgKey >> 4) & 15) * 16 + 8, (bgKey & 15) * 16 + 8];
    const y0 = band ? Math.floor(h * band.y0) : 0;
    const y1 = band ? Math.min(h, Math.ceil(h * band.y1)) : h;
    let ink = 0;
    let total = 0;
    for (let y = y0; y < y1; y++) {
      for (let x = 0; x < w; x++) {
        const o = (y * w + x) * 4;
        total += 1;
        const d = Math.max(
          Math.abs(data[o] - bg[0]),
          Math.abs(data[o + 1] - bg[1]),
          Math.abs(data[o + 2] - bg[2]),
          Math.abs(data[o + 3] - bg[3])
        );
        if (d > INK_DIFF) ink += 1;
      }
    }
    return { w, h, ratio: total ? ink / total : 0, ink, total };
  };
  window.__fc = {
    now: () => performance.now(),
    txt,
    visible,
    leftTab,
    leftSelected,
    tree,
    rows,
    row,
    rowInfo,
    rowsInfo: () => rows().map(rowInfo),
    filesHeaderText,
    filesText: () => txt(filesRoot()),
    refreshButton,
    reviewTablist,
    reviewTabs,
    liveTab,
    fileTab,
    tabsInfo,
    selectedTab,
    currentPanel,
    panelText,
    closeButtonFor,
    vscodeLink,
    viewer,
    contentScroller,
    treeScroller,
    contract,
    inkStats,
  };
}

// ---------------------------------------------------------------------------
// 網路與 console 記錄
// ---------------------------------------------------------------------------

function classify(url) {
  let p;
  try {
    p = new URL(url).pathname;
  } catch {
    return 'other';
  }
  if (/^\/api\/runtimes\/[^/]+\/panes\/[^/]+\/output$/.test(p)) return 'output';
  if (/^\/api\/runtimes\/[^/]+\/panes\/[^/]+\/root$/.test(p)) return 'root';
  const m = /^\/api\/files\/[^/]+\/[^/]+\/(list|meta|render|raw)(\/|$)/.exec(p);
  if (m) return m[1];
  if (p === '/api/state') return 'state';
  return 'other';
}
const isFileEndpoint = (kind) => ['root', 'list', 'meta', 'render', 'raw'].includes(kind);

async function recordNetwork(cdp) {
  const reqs = [];
  const byKey = new Map();
  const sessions = []; // auto-attach 的子 session（OOPIF、worker）：{ sid, type, url }
  cdp.onEvent('Network.requestWillBeSent', (p, sid) => {
    const r = { key: `${sid || ''}|${p.requestId}`, url: p.request.url, kind: classify(p.request.url), at: Date.now(), status: null, failed: false, session: sid };
    byKey.set(r.key, r);
    reqs.push(r);
  });
  cdp.onEvent('Network.responseReceived', (p, sid) => {
    const r = byKey.get(`${sid || ''}|${p.requestId}`);
    if (r) {
      r.status = p.response.status;
      r.headers = Object.fromEntries(Object.entries(p.response.headers || {}).map(([k, v]) => [k.toLowerCase(), v]));
    }
  });
  cdp.onEvent('Network.loadingFailed', (p, sid) => {
    const r = byKey.get(`${sid || ''}|${p.requestId}`);
    if (r) {
      r.failed = true;
      r.errorText = p.errorText;
    }
  });
  // OOPIF（例如 sandbox iframe）與 worker 自動 attach：啟用網路事件後放行。
  cdp.onEvent('Target.attachedToTarget', (p) => {
    const sid = p.sessionId;
    sessions.push({ sid, type: p.targetInfo && p.targetInfo.type, url: p.targetInfo && p.targetInfo.url });
    (async () => {
      await cdp.send('Network.enable', {}, sid);
      await cdp.send('Runtime.runIfWaitingForDebugger', {}, sid);
    })().catch(() => {});
  });
  await cdp.send('Network.enable');
  await cdp.send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
  return {
    reqs,
    sessions,
    since: (t, kind) => reqs.filter((r) => r.at >= t && (!kind || r.kind === kind)),
  };
}

async function recordConsole(cdp) {
  const entries = [];
  cdp.onEvent('Runtime.consoleAPICalled', (p) => {
    entries.push({ at: Date.now(), level: p.type, text: (p.args || []).map((a) => a.value !== undefined ? String(a.value) : a.description || '').join(' ') });
  });
  cdp.onEvent('Runtime.exceptionThrown', (p) => {
    const d = p.exceptionDetails || {};
    entries.push({ at: Date.now(), level: 'exception', text: d.exception && d.exception.description ? d.exception.description : d.text });
  });
  cdp.onEvent('Log.entryAdded', (p) => {
    entries.push({ at: Date.now(), level: p.entry.level, text: p.entry.text, source: p.entry.source });
  });
  await cdp.send('Runtime.enable');
  await cdp.send('Log.enable');
  return entries;
}

// 以 Fetch 在 Response 階段攔截某個檔案的中繼資料回應，把 vscode_uri 換成指定值（「WSL 檔案」段：
// ui_preview 沒有 WSL runtime）。回傳攔截次數的讀取函式。
async function installMetaRewrite(cdp, relPath, newUri) {
  let count = 0;
  const suffix = `/meta/${relPath.split('/').map(encodeURIComponent).join('/')}`;
  cdp.onEvent('Fetch.requestPaused', (p) => {
    (async () => {
      let pathname = '';
      try {
        pathname = new URL(p.request.url).pathname;
      } catch {
        // 不處理。
      }
      if (p.responseStatusCode === undefined || !pathname.endsWith(suffix) || p.responseStatusCode !== 200) {
        await cdp.send('Fetch.continueRequest', { requestId: p.requestId });
        return;
      }
      const body = await cdp.send('Fetch.getResponseBody', { requestId: p.requestId });
      const raw = body.result.base64Encoded ? Buffer.from(body.result.body, 'base64').toString('utf8') : body.result.body;
      const json = JSON.parse(raw);
      json.vscode_uri = newUri;
      count += 1;
      const headers = (p.responseHeaders || []).filter((h) => h.name.toLowerCase() !== 'content-length');
      await cdp.send('Fetch.fulfillRequest', {
        requestId: p.requestId,
        responseCode: 200,
        responseHeaders: headers,
        body: Buffer.from(JSON.stringify(json), 'utf8').toString('base64'),
      });
    })().catch((e) => log(`Fetch 攔截處理例外：${e.message}`));
  });
  await cdp.send('Fetch.enable', { patterns: [{ urlPattern: `*${suffix}`, requestStage: 'Response' }] });
  return { count: () => count };
}

// ---------------------------------------------------------------------------
// 段落共用：啟動 preview＋chrome、導覽、等首份投影；收尾
// ---------------------------------------------------------------------------

// 「第一份真投影已經畫出」（同 visual-check.js waitForFirstProjection，Ruling R22）。
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
    ctx.chrome = await startChrome(pickPort(19310, [ctx.preview.port]), 'about:blank', label, opts.windowSize);
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
const encRel = (rel) => rel.split('/').map(encodeURIComponent).join('/');
async function metaOf(ctx, rootId, rel) {
  return apiJson(ctx, `/api/files/${RUNTIME}/${rootId}/meta/${encRel(rel)}`);
}

async function contractDump(ctx) {
  const c = await ctx.cdp.run(() => (window.__fc ? window.__fc.contract() : { helpers: false })).catch((e) => ({ error: e.message }));
  return JSON.stringify(c);
}

// --- 使用者操作（全部經由畫面：點擊真的滑鼠事件）---

async function selectPane(ctx, pane) {
  const sel = `.pane-row[data-runtime="${RUNTIME}"][data-pane="${pane}"]`;
  need(await ctx.cdp.clickEl((s) => document.querySelector(s), [sel], `pane 列 ${RUNTIME}/${pane}`), `點 pane 列 ${RUNTIME}/${pane}`);
  const ok = await ctx.cdp.poll((s) => {
    const r = document.querySelector(s);
    return !!r && r.classList.contains('selected');
  }, [sel], UI_TIMEOUT_MS);
  need(!!ok, `pane 列 ${RUNTIME}/${pane} 出現選定標示`);
}

async function switchLeftTab(ctx, name) {
  const found = await ctx.cdp.poll((n) => !!window.__fc.leftTab(n), [name], UI_TIMEOUT_MS);
  if (!found) {
    need(false, `找不到左欄分頁「${name}」（契約 C1：#files 內 role="tablist" 的 role="tab"，可見文字「${name}」）；目前 DOM：${await contractDump(ctx)}`);
  }
  need(await ctx.cdp.clickEl((n) => window.__fc.leftTab(n), [name], `左欄分頁「${name}」`), `點左欄分頁「${name}」`);
  const sel = await ctx.cdp.poll((n) => {
    const t = window.__fc.leftTab(n);
    return !!t && t.getAttribute('aria-selected') === 'true';
  }, [name], UI_TIMEOUT_MS);
  need(!!sel, `左欄分頁「${name}」成為目前分頁（aria-selected="true"）`);
}

async function waitRow(ctx, rel) {
  const ok = await ctx.cdp.poll((p) => {
    const r = window.__fc.row(p);
    return !!r && window.__fc.visible(r);
  }, [rel], UI_TIMEOUT_MS);
  if (!ok) {
    need(false, `檔案樹應該出現可見的列 title="${rel}"（契約 C2：#files 內 role="tree" 的 role="treeitem"，title＝完整相對路徑）；目前 DOM：${await contractDump(ctx)}`);
  }
}

// 選定 pane（預設 wJ:p4，根目錄 review-repo）並切到左欄「檔案」，等到根目錄第一層出現。
async function openTree(ctx, pane = PANE_REVIEW, probeRow = 'README.md') {
  await selectPane(ctx, pane);
  await switchLeftTab(ctx, '檔案');
  await waitRow(ctx, probeRow);
}

async function expandDir(ctx, rel) {
  await waitRow(ctx, rel);
  const state = await ctx.cdp.run((p) => window.__fc.row(p).getAttribute('aria-expanded'), rel);
  need(state === 'true' || state === 'false', `資料夾列 ${rel} 帶 aria-expanded（實際 ${JSON.stringify(state)}）`);
  if (state === 'true') return;
  need(await ctx.cdp.clickEl((p) => window.__fc.row(p), [rel], `資料夾列 ${rel}`), `點資料夾列 ${rel}`);
  const ok = await ctx.cdp.poll((p) => window.__fc.row(p) && window.__fc.row(p).getAttribute('aria-expanded') === 'true', [rel], UI_TIMEOUT_MS);
  need(!!ok, `資料夾列 ${rel} 展開（aria-expanded="true"）`);
}

async function openFile(ctx, rel) {
  const parts = rel.split('/');
  for (let i = 1; i < parts.length; i++) await expandDir(ctx, parts.slice(0, i).join('/'));
  await waitRow(ctx, rel);
  need(await ctx.cdp.clickEl((p) => window.__fc.row(p), [rel], `檔案列 ${rel}`), `點檔案列 ${rel}`);
  const ok = await ctx.cdp.poll((p) => {
    const t = window.__fc.fileTab(p);
    return !!t && t.getAttribute('aria-selected') === 'true';
  }, [rel], UI_TIMEOUT_MS);
  if (!ok) {
    need(false, `點 ${rel} 後應該出現並選定它的檔案分頁（契約 C3：#review 內 role="tablist" 的 role="tab"，data-path="${rel}"，aria-selected="true"）；目前 DOM：${await contractDump(ctx)}`);
  }
}

async function clickTab(ctx, which) {
  const isLive = which === 'LIVE';
  const finder = isLive ? () => window.__fc.liveTab() : (p) => window.__fc.fileTab(p);
  const args = isLive ? [] : [which];
  const desc = isLive ? 'Live Output 分頁' : `檔案分頁 ${which}`;
  const exists = await ctx.cdp.poll(finder, args, UI_TIMEOUT_MS);
  if (!exists) need(false, `找不到${desc}（契約 C3）；目前 DOM：${await contractDump(ctx)}`);
  need(await ctx.cdp.clickEl(finder, args, desc), `點${desc}`);
  const sel = await ctx.cdp.poll(
    isLive
      ? () => !!window.__fc.liveTab() && window.__fc.liveTab().getAttribute('aria-selected') === 'true'
      : (p) => !!window.__fc.fileTab(p) && window.__fc.fileTab(p).getAttribute('aria-selected') === 'true',
    args,
    UI_TIMEOUT_MS
  );
  need(!!sel, `${desc}成為目前分頁（aria-selected="true"）`);
}

const tabsInfo = (ctx) => ctx.cdp.run(() => window.__fc.tabsInfo());
const panelText = (ctx) => ctx.cdp.run(() => window.__fc.panelText());
async function waitPanelText(ctx, needle, timeoutMs = UI_TIMEOUT_MS) {
  return ctx.cdp.poll((n) => window.__fc.panelText().includes(n), [needle], timeoutMs);
}

// 只看目前分頁檢視器（契約 C6 的 [data-viewer] 元素）內的可見文字，不含工具列與狀態列。
async function waitViewerText(ctx, needle, timeoutMs = UI_TIMEOUT_MS) {
  return ctx.cdp.poll((n) => !!window.__fc.viewer() && window.__fc.txt(window.__fc.viewer()).includes(n), [needle], timeoutMs);
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
// self/鷹架
// ---------------------------------------------------------------------------

// 頁面端：用 vendored pdf.js 畫 report.pdf 每一頁，量「非背景像素比例」，作為中文 PDF 段判準的
// 正／負對照（不是在驗前端，是在驗本檔的偵測器與門檻）。
async function calibratePdfDetector(rawUrl, band) {
  const out = { pages: [] };
  const pdfjs = await import('/vendor/pdfjs/pdf.min.mjs');
  pdfjs.GlobalWorkerOptions.workerSrc = '/vendor/pdfjs/pdf.worker.min.mjs';
  const doc = await pdfjs.getDocument({
    url: rawUrl,
    cMapUrl: '/vendor/pdfjs/cmaps/',
    cMapPacked: true,
    standardFontDataUrl: '/vendor/pdfjs/standard_fonts/',
    disableRange: true,
    disableStream: true,
  }).promise;
  out.numPages = doc.numPages;
  for (let i = 1; i <= doc.numPages; i++) {
    const page = await doc.getPage(i);
    const vp1 = page.getViewport({ scale: 1 });
    const vp = page.getViewport({ scale: 800 / vp1.width });
    const canvas = document.createElement('canvas');
    canvas.width = Math.round(vp.width);
    canvas.height = Math.round(vp.height);
    await page.render({ canvas, viewport: vp }).promise;
    const whole = window.__fc.inkStats(canvas, null);
    const titleBand = window.__fc.inkStats(canvas, band);
    const rec = { whole: whole.ratio, band: titleBand.ratio, w: canvas.width, h: canvas.height };
    if (i === 1) {
      const c2 = canvas.getContext('2d');
      c2.fillStyle = '#ffffff';
      const y0 = Math.floor(canvas.height * band.y0);
      c2.fillRect(0, y0, canvas.width, Math.ceil(canvas.height * band.y1) - y0 + 1);
      rec.bandErased = window.__fc.inkStats(canvas, band).ratio;
    }
    out.pages.push(rec);
  }
  const white = document.createElement('canvas');
  white.width = 800;
  white.height = 1035;
  const wc = white.getContext('2d');
  wc.fillStyle = '#ffffff';
  wc.fillRect(0, 0, 800, 1035);
  out.blankWhite = window.__fc.inkStats(white, null).ratio;
  const clear = document.createElement('canvas');
  clear.width = 800;
  clear.height = 1035;
  out.blankTransparent = window.__fc.inkStats(clear, null).ratio;
  return out;
}

// 頁面端：臨時插入一份照契約 C1–C6 寫的最小 DOM，驗 window.__fc 的定位規則（正對照），再移除。
// file-review task 4.1 起前端已有真的 #files／#review（id 會跟合成 DOM 撞），先把真的節點暫時換成
// 註解佔位、量完再原地放回（節點本身不重建，前端模組持有的參照不受影響）；`restored` 確認放回。
function syntheticContractProbe() {
  const had = { files: !!document.getElementById('files'), review: !!document.getElementById('review') };
  const parked = [];
  for (const id of ['files', 'review']) {
    const el = document.getElementById(id);
    if (el) {
      const mark = document.createComment(`fc-park-${id}`);
      el.replaceWith(mark);
      parked.push({ el, mark });
    }
  }
  const host = document.createElement('div');
  host.innerHTML =
    '<div id="files"><div role="tablist"><button role="tab" aria-selected="false">Project</button>' +
    '<button role="tab" aria-selected="true">檔案</button></div>' +
    '<div>review-repo <span>win</span> <button>重新整理</button></div>' +
    '<div role="tree"><div role="treeitem" aria-level="1" aria-expanded="true" title="src" tabindex="0">src</div>' +
    '<div role="treeitem" aria-level="2" title="src/main.rs" tabindex="-1">main.rs</div></div></div>' +
    '<div id="review"><div role="tablist">' +
    '<div role="tab" aria-selected="false" aria-controls="fc-p0">Live Output</div>' +
    '<span><div role="tab" aria-selected="true" aria-controls="fc-p1" data-path="README.md" title="README.md — review-repo">README.md</div>' +
    '<button aria-label="關閉 README.md">×</button></span></div>' +
    '<div role="tabpanel" id="fc-p0" hidden></div>' +
    '<div role="tabpanel" id="fc-p1"><a href="vscode://file/X:/r/README.md">在 VS Code 開啟</a>' +
    '<div data-viewer="markdown" style="height:40px;overflow:auto"><div style="height:400px">內容</div></div></div></div>';
  document.body.appendChild(host);
  const fc = window.__fc;
  const r = {
    leftTab: !!fc.leftTab('檔案'),
    leftSelected: fc.leftSelected(),
    header: fc.filesHeaderText(),
    refresh: !!fc.refreshButton(),
    row: fc.row('src/main.rs') ? fc.rowInfo(fc.row('src/main.rs')) : null,
    srcExpanded: fc.row('src') ? fc.row('src').getAttribute('aria-expanded') : null,
    tabs: fc.tabsInfo(),
    panelHasViewer: !!fc.viewer(),
    close: !!fc.closeButtonFor(fc.fileTab('README.md')),
    href: fc.vscodeLink() ? fc.vscodeLink().getAttribute('href') : null,
    scroller: !!fc.contentScroller(),
  };
  host.remove();
  r.cleaned = !document.getElementById('files') && !document.getElementById('review');
  for (const p of parked) p.mark.replaceWith(p.el);
  r.had = had;
  r.restored = parked.every((p) => p.el.isConnected) && (!had.files || !!document.getElementById('files')) && (!had.review || !!document.getElementById('review'));
  return r;
}

async function segSelfScaffold() {
  const fixtureHashBefore = hashTree(FIXTURE_SRC);
  let ctx = null;
  try {
    ctx = await openCockpit('self');
    const { cdp, preview } = ctx;
    // 1. 暫存副本路徑（ui_preview stdout）
    check(!!preview.reviewRepo && fs.existsSync(path.join(preview.reviewRepo, 'README.md')), `讀到 ui_preview 印出的 review-repo 暫存路徑且含 README.md（${preview.reviewRepo}）`);
    check(!!preview.otherRepo && fs.existsSync(path.join(preview.otherRepo, 'README.md')), `讀到 other-repo 暫存路徑且含 README.md（${preview.otherRepo}）`);
    check(
      path.basename(preview.tempRoot).startsWith(PREVIEW_TEMP_PREFIX) && !path.resolve(preview.reviewRepo).toLowerCase().startsWith(REPO.toLowerCase()),
      `暫存副本在 repo 之外、目錄名以 ${PREVIEW_TEMP_PREFIX} 開頭（${preview.tempRoot}）`
    );
    // 2. 假 pane 的根目錄就是暫存副本
    const root = await rootInfo(ctx, PANE_REVIEW);
    check(
      root.name === 'review-repo' && root.is_git === true && path.resolve(root.root_path).toLowerCase() === path.resolve(preview.reviewRepo).toLowerCase(),
      `${PANE_REVIEW} 的根目錄＝review-repo 暫存副本（root_path ${root.root_path}）`
    );
    const other = await rootInfo(ctx, PANE_OTHER);
    check(other.name === 'other-repo' && other.root_id !== root.root_id, `${PANE_OTHER} 的根目錄是另一個根目錄 other-repo`);
    // 3. 網路記錄與依 URL 分類
    const t0 = Date.now();
    await cdp.run(async (rid) => {
      await fetch('/api/state');
      await fetch('/api/files/win/' + rid + '/meta/README.md');
      await fetch('/api/runtimes/win/panes/' + encodeURIComponent('wJ:p1') + '/output');
      return true;
    }, root.root_id);
    await sleep(300);
    const recent = ctx.net.since(t0);
    check(recent.some((r) => r.kind === 'state' && r.status === 200), `網路記錄：/api/state 被分類為 state、status 200`);
    check(recent.some((r) => r.kind === 'meta' && r.status === 200), `網路記錄：中繼資料請求被分類為 meta、status 200`);
    check(recent.some((r) => r.kind === 'output'), `網路記錄：輸出請求被分類為 output`);
    // 4. Fetch 攔截改寫中繼資料
    const rw = await installMetaRewrite(cdp, 'README.md', 'vscode://files-check-selftest/rewritten');
    const got = await cdp.run(async (rid) => (await (await fetch('/api/files/win/' + rid + '/meta/README.md')).json()).vscode_uri, root.root_id);
    check(got === 'vscode://files-check-selftest/rewritten' && rw.count() >= 1, `Fetch 攔截能改寫中繼資料的 vscode_uri（頁面讀到 ${got}，攔截 ${rw.count()} 次）`);
    await cdp.send('Fetch.disable');
    // 5. message 監聽器
    await cdp.run(() => {
      window.postMessage('files-check-probe', '*');
      return true;
    });
    check(!!(await cdp.poll(() => window.__fcMessages.includes('files-check-probe'), [], 2000)), 'message 監聽器記得到 postMessage（「HTML 內的腳本不執行」段的偵測器）');
    // 6. 改寫暫存副本並還原
    const target = path.join(preview.reviewRepo, 'long.md');
    const orig = fs.readFileSync(target);
    const m0 = (await metaOf(ctx, root.root_id, 'long.md')).body;
    fs.appendFileSync(target, '\n\nfiles-check self-test marker\n');
    const m1 = (await metaOf(ctx, root.root_id, 'long.md')).body;
    check(!!m0 && !!m1 && (m1.size !== m0.size || m1.modified_ms !== m0.modified_ms), `改寫暫存副本 long.md 後中繼資料改變（size ${m0 && m0.size} → ${m1 && m1.size}）`);
    fs.writeFileSync(target, orig);
    const m2 = (await metaOf(ctx, root.root_id, 'long.md')).body;
    check(!!m2 && m2.size === m0.size && Buffer.compare(fs.readFileSync(target), orig) === 0, '還原暫存副本 long.md 後內容與大小回到原值');
    // 7. PDF 像素偵測器校準（vendored pdf.js）
    const rawUrl = `/api/files/${RUNTIME}/${root.root_id}/raw/report.pdf`;
    const cal = await cdp.run(calibratePdfDetector, rawUrl, PDF_TITLE_BAND);
    log(`PDF 偵測器校準：${JSON.stringify(cal)}`);
    check(cal.numPages === 3, `fixture report.pdf 為 3 頁（pdf.js 解析 ${cal.numPages} 頁）`);
    check(cal.blankWhite < PDF_PAGE_INK_MIN && cal.blankTransparent < PDF_PAGE_INK_MIN, `負對照：全白與全透明空白頁的非背景比例（${cal.blankWhite}／${cal.blankTransparent}）低於整頁門檻 ${PDF_PAGE_INK_MIN}`);
    check(cal.pages.length === 3 && cal.pages.every((p) => p.whole > PDF_PAGE_INK_MIN), `正對照：pdf.js 畫出的 3 頁整頁非背景比例都高於 ${PDF_PAGE_INK_MIN}（${cal.pages.map((p) => p.whole.toFixed(4)).join('／')}）`);
    check(cal.pages[0] && cal.pages[0].band > PDF_TITLE_INK_MIN * 2, `正對照：第 1 頁標題帶非背景比例 ${cal.pages[0] && cal.pages[0].band.toFixed(4)} 至少是門檻 ${PDF_TITLE_INK_MIN} 的兩倍（門檻留有縮放差異餘裕）`);
    check(cal.pages[0] && cal.pages[0].bandErased < PDF_TITLE_INK_MIN, `負對照：標題帶塗白後非背景比例 ${cal.pages[0] && cal.pages[0].bandErased} 低於門檻`);
    // 8. 前端契約定位規則的正對照（合成 DOM）
    const real = await cdp.run(() => window.__fc.contract());
    log(`目前前端的契約狀態（file-review task 4.1 起有 #files／#review，合成 DOM 量測期間暫時移開）：${JSON.stringify(real)}`);
    const sc = await cdp.run(syntheticContractProbe);
    check(
      sc.restored &&
        sc.leftTab &&
        sc.leftSelected === '檔案' &&
        /review-repo/.test(sc.header) &&
        /win/.test(sc.header) &&
        sc.refresh &&
        sc.row &&
        sc.row.visible &&
        sc.srcExpanded === 'true' &&
        sc.tabs.length === 2 &&
        sc.tabs[0].live &&
        sc.tabs[1].path === 'README.md' &&
        sc.tabs[1].selected &&
        sc.panelHasViewer &&
        sc.close &&
        sc.href === 'vscode://file/X:/r/README.md' &&
        sc.scroller &&
        sc.cleaned,
      `契約定位規則對合成 DOM 全部找得到（${JSON.stringify(sc)}）`
    );
    // 10.（fix round 1）檔案樹「節點沒被換掉」偵測器的正負對照（合成 DOM；真的檔案樹由 4.2 實作）。
    await cdp.eval(`window.treeSnapshot = ${treeSnapshot.toString()}; window.treeCompare = ${treeCompare.toString()}; true`);
    const td = await cdp.run(syntheticTreeDetectorProbe);
    const allSame = (c) => c.sameTree && c.sameRows && c.sameDescendants && c.sameExpanded && c.scSame && c.scrollTop === c.expectScrollTop;
    check(allSame(td.untouched), `檔案樹偵測器正對照：什麼都不動時全部判定相同（${JSON.stringify(td.untouched)}）`);
    check(td.rowInnerReplaced.sameRows && !td.rowInnerReplaced.sameDescendants, `檔案樹偵測器負對照：只重設某一列的 innerHTML（列本身不變）必須被抓到（${JSON.stringify(td.rowInnerReplaced)}）`);
    check(!td.scrollerReplaced.scSame && !td.scrollerReplaced.sameTree, `檔案樹偵測器負對照：捲動容器換成內容相同的新節點必須被抓到（${JSON.stringify(td.scrollerReplaced)}）`);
    // 11.（fix round 2，控制端裁決 R23）輸出請求跨切換分析：純函式的案例＋在目前前端上以真的 Live Output tab
    // 做正負對照（a）–（d）。
    const R = (id, start, end, live, endLive, via) => ({ id, via: via || 'fetch', start, end, at: { live }, endAt: end === null ? null : { live: endLive } });
    const cases = [
      { name: '合規：一個既有請求在切走後完成、切回點擊後 5 ms 發出', r: analyzeOutputAcrossSwitch([R(1, 900, 1200, 'true', 'false'), R(2, 11005, null, 'true')], 11000), ok: (r) => r.completedWhileAway.length === 1 && r.startedWhileAway.length === 0 && r.immediateMs === 5 },
      { name: 'Codex 重現：切回點擊 11000、同步設定後立即 fetch（start 11000.05）不誤報', r: analyzeOutputAcrossSwitch([R(1, 11000.05, null, 'true')], 11000), ok: (r) => r.startedWhileAway.length === 0 && r.immediateMs !== null && r.immediateMs <= 1 },
      { name: '違規：切走後才發出一個新請求（即使在切走後 0.05 ms）', r: analyzeOutputAcrossSwitch([R(1, 1000.05, 1100, 'false', 'false'), R(2, 11005, null, 'true')], 11000), ok: (r) => r.startedWhileAway.length === 1 },
      { name: '違規：切走後完成了兩個既有請求', r: analyzeOutputAcrossSwitch([R(1, 800, 1100, 'true', 'false'), R(2, 900, 1300, 'true', 'false')], 11000), ok: (r) => r.completedWhileAway.length === 2 },
      { name: '違規：切回後沒有請求', r: analyzeOutputAcrossSwitch([R(1, 900, 950, 'true', 'true')], 11000), ok: (r) => r.immediateMs === null },
      { name: '違規：切回後 900 ms 才發出（只是等原本的輪詢計時）', r: analyzeOutputAcrossSwitch([R(1, 11900, null, 'true')], 11000), ok: (r) => r.immediateMs === 900 && r.immediateMs > IMMEDIATE_MS },
    ];
    for (const c of cases) check(c.ok(c.r), `analyzeOutputAcrossSwitch：${c.name}（${JSON.stringify(c.r)}）`);
    // 真頁面（沒有選定 pane，output.js 不輪詢）：直接改真的 Live Output tab 的 aria-selected 模擬切換（4.3 之前
    // 還開不了檔案分頁），全部在同一個同步區塊內「設定屬性後立即發請求」，不留任何 await 間隔：
    //   (b) 設成 "false" 後立即 fetch → 必須被判為切走期間發出；
    //   (c) 仍在切走期間以 XHR 發輸出請求 → 必須被判為切走期間發出（via xhr）；
    //   (a) 在點 Live Output tab 的 click handler 裡把屬性設回 "true" 並立即 fetch → 不得誤報，且判為立即。
    // 前端的 selectReviewTab 對「已是目前分頁」的 tab 直接 return，點擊不改前端狀態；量完 aria-selected 為 "true"。
    const live = await cdp.run(async () => {
      const tab = window.__fc.liveTab();
      if (!tab || tab.getAttribute('aria-selected') !== 'true' || document.querySelector('.pane-row.selected')) return { error: '前提不符：需要 Live Output tab 為目前分頁且沒有選定 pane' };
      const url = '/api/runtimes/win/panes/' + encodeURIComponent('wJ:p1') + '/output';
      const n0 = window.__fcRequests.length;
      tab.setAttribute('aria-selected', 'false');
      const pB = fetch(url).catch(() => null);
      const x = new XMLHttpRequest();
      const pC = new Promise((r) => x.addEventListener('loadend', r));
      x.open('GET', url);
      x.send();
      await Promise.all([pB, pC]);
      let pA = null;
      const onClick = () => {
        tab.setAttribute('aria-selected', 'true');
        pA = fetch(url).catch(() => null);
      };
      tab.addEventListener('click', onClick, { once: true });
      tab.click();
      await pA;
      return { n0, restored: tab.getAttribute('aria-selected') === 'true' };
    });
    need(!live.error, `真頁面正負對照前提（${JSON.stringify(live)}）`);
    const pr = await pageRequests(ctx);
    const mine = pr.recs.slice(live.n0).filter((r) => classify(r.url) === 'output');
    const la = analyzeOutputAcrossSwitch(mine, pr.liveClicks.length ? pr.liveClicks[pr.liveClicks.length - 1] : null);
    log(`真頁面對照的請求記錄：${JSON.stringify(mine.map((r) => ({ id: r.id, via: r.via, at: r.at, endAt: r.endAt })))}；分析 ${JSON.stringify(la)}`);
    check(mine.length === 3, `真頁面對照：頁面端記到 3 個輸出請求（fetch、XHR、fetch；實際 ${mine.length}）`);
    check(la.startedWhileAway.some((s) => s.endsWith(':fetch')), `(b) 同步設成切走後立即 fetch 被抓到（${JSON.stringify(la.startedWhileAway)}）`);
    check(la.startedWhileAway.some((s) => s.endsWith(':xhr')), `(c) 切走期間以 XHR 發的輸出請求被抓到（${JSON.stringify(la.startedWhileAway)}）`);
    check(la.startedWhileAway.length === 2 && la.immediateMs !== null && la.immediateMs <= IMMEDIATE_MS, `(a) 同步設回 Live Output 後立即 fetch 不誤報、判為立即（違規 ${la.startedWhileAway.length} 筆只有 (b)(c)；點擊後 ${la.immediateMs} ms）`);
    check(live.restored, 'Live Output tab 的 aria-selected 已還原為 "true"');
    // (d) CDP 總數健全檢查：目前沒有任何輪詢，頁面端與 CDP 的輸出請求數應相等（正對照）；再以 <img> 發一個不經
    // fetch／XHR 的輸出請求，CDP 看得到、頁面端記不到，檢查必須判 FAIL（負對照）。
    const s0 = await cdpPageCountSanity(ctx, 'output');
    check(s0.ok, `(d) 正對照：沒有未包到的途徑時 CDP 與頁面端輸出請求數相等（${JSON.stringify(s0)}）`);
    await cdp.run(() => {
      new Image().src = '/api/runtimes/win/panes/' + encodeURIComponent('wJ:p1') + '/output';
      return true;
    });
    await sleep(300);
    const s1 = await cdpPageCountSanity(ctx, 'output');
    check(!s1.ok && s1.cdp === s1.page + 1, `(d) 負對照：以 <img> 發的輸出請求讓 CDP 總數多 1，健全檢查判 FAIL（${JSON.stringify(s1)}）`);
  } finally {
    await closeCockpit(ctx);
  }
  // 9. 收尾衛生
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
    { arg: ' file-review/中文 PDF , self/鷹架 ', ok: true, codes: ['file-review/中文 PDF', 'self/鷹架'] },
    { arg: 'live-output/', ok: true, codes: known.filter((c) => c.startsWith('live-output/')) },
    { arg: 'file-review/中文PDF', ok: false, mention: 'file-review/中文PDF' },
    { arg: 'nope/', ok: false, mention: 'nope/' },
    { arg: 'Self/鷹架', ok: false, mention: 'Self/鷹架' },
    { arg: '', ok: false },
    { arg: ',', ok: false },
  ];
  for (const c of cases) {
    const r = parseSegmentArg(c.arg, known);
    const good =
      r.ok === c.ok &&
      (c.ok ? JSON.stringify(r.codes) === JSON.stringify(c.codes) : typeof r.message === 'string' && (!c.mention || r.message.includes(c.mention)));
    check(good, `parseSegmentArg(${JSON.stringify(c.arg)}) → ${c.ok ? '通過' : '拒絕'}（實際 ${JSON.stringify(r).slice(0, 160)}）`);
  }
  check(known.filter((c) => c.startsWith('live-output/')).length === 3, 'live-output/ 前綴恰好選中 3 段');
  for (const arg of ['file-review/不存在', '']) {
    const r = spawnSync(process.execPath, [__filename, arg], { encoding: 'utf8', timeout: 30000, windowsHide: true });
    const out = `${r.stdout || ''}${r.stderr || ''}`;
    check(
      r.status === 2 && !out.includes('RESULT: PASS') && out.includes('RESULT: FAIL (段落代號)') && (arg === '' || out.includes(arg)) && !out.includes('啟動'),
      `node files-check.js ${JSON.stringify(arg)}：exit 2、不印 PASS、指出問題、不啟動行程（exit ${r.status}；輸出 ${JSON.stringify(out.trim().slice(0, 160))}）`
    );
  }
}

// ---------------------------------------------------------------------------
// file-review：左欄檔案樹
// ---------------------------------------------------------------------------

// git-review task 4.2：WHEN 載入頁面 THEN 左欄頂端依序為「Project」「檔案」「變更」三個分頁，目前為
// 「Project」；在分頁上按右方向鍵兩次再按 Enter，目前分頁為「變更」。放在這裡（不放
// git-check.js）：這一段只驗左欄 tablist 本身的通用機制（三個分頁按 DOM 順序排列、方向鍵移動焦點、
// Enter 選定），跟 git 後端或「變更」面板的內容完全無關，files-check.js 已經有現成的左欄分頁夾具
// （leftSelected()、switchLeftTab() 的鍵盤變體），不必為了這一段另外啟動 git-check.js 的機器。
async function segLeftThreeTabs() {
  await withCockpit('left-three-tabs', {}, async (ctx) => {
    const initial = await ctx.cdp.run(() => Array.from(document.querySelectorAll('#files [role="tablist"] [role="tab"]')).map((t) => t.textContent.trim()));
    check(JSON.stringify(initial) === JSON.stringify(['Project', '檔案', '變更']), `左欄頂端依序為「Project」「檔案」「變更」三個分頁（實際 ${JSON.stringify(initial)}）`);
    const currentInitial = await ctx.cdp.run(() => window.__fc.leftSelected());
    check(currentInitial === 'Project', `載入頁面時目前分頁為「Project」（實際「${currentInitial}」）`);
    const focused = await ctx.cdp.run(() => {
      const el = document.getElementById('files-tab-projects');
      el.focus();
      return document.activeElement === el;
    });
    need(focused, '前置：鍵盤焦點可以移到「Project」分頁');
    await ctx.cdp.pressKey('ArrowRight', 'ArrowRight', 39);
    await ctx.cdp.pressKey('ArrowRight', 'ArrowRight', 39);
    const focusedName = await ctx.cdp.run(() => (document.activeElement ? document.activeElement.textContent.trim() : null));
    check(focusedName === '變更', `按右方向鍵兩次後鍵盤焦點在「變更」分頁（實際「${focusedName}」）`);
    await ctx.cdp.pressKey('Enter', 'Enter', 13, '\r');
    const sel = await ctx.cdp.poll(() => window.__fc.leftSelected(), [], UI_TIMEOUT_MS);
    check(sel === '變更', `按 Enter 後目前分頁為「變更」（實際「${sel}」）`);
  });
}

// GIVEN 已選定 w1:p1（wJ:p4，根目錄 review-repo）WHEN 點左欄「檔案」THEN 顯示 repo 的檔案樹，第一層為
// 根目錄的子項目。第一層以列目錄端點（服務端）的回應為準，比對集合。
async function segSwitchToFilesTab() {
  await withCockpit('tree-switch', {}, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    const list = await apiJson(ctx, `/api/files/${RUNTIME}/${root.root_id}/list`);
    need(list.status === 200, `列目錄端點回 200（前置）`);
    const expected = list.body.entries.map((e) => e.name).sort();
    await selectPane(ctx, PANE_REVIEW);
    await switchLeftTab(ctx, '檔案');
    await waitRow(ctx, 'README.md');
    await sleep(300);
    const header = await ctx.cdp.run(() => window.__fc.filesHeaderText());
    check(header.includes('review-repo') && header.includes(RUNTIME), `檔案樹頂端顯示根目錄 name「review-repo」與 runtime id「${RUNTIME}」（實際「${header}」）`);
    check(!!(await ctx.cdp.run(() => !!window.__fc.refreshButton())), '檔案樹頂端有「重新整理」按鈕');
    const rows = await ctx.cdp.run(() => window.__fc.rowsInfo());
    const top = rows.filter((r) => r.title && !r.title.includes('/')).map((r) => r.title).sort();
    check(JSON.stringify(top) === JSON.stringify(expected), `第一層列（title 不含 /）＝根目錄的子項目（預期 ${JSON.stringify(expected)}，實際 ${JSON.stringify(top)}）`);
    check(rows.every((r) => r.visible && r.text.includes(r.title.split('/').pop())), `每一列可見且顯示名稱（${JSON.stringify(rows.map((r) => r.text))}）`);
  });
}

// GIVEN 沒有選定 pane、沒有打開任何檔案分頁 WHEN 切到「檔案」THEN 顯示提示選 pane 的空狀態，頁面沒有
// 發出任何檔案端點的請求（從導覽開始算，含根目錄查詢）。
async function segNoPaneSelected() {
  await withCockpit('tree-nopane', {}, async (ctx) => {
    const pre = await ctx.cdp.run(() => ({ selected: document.querySelectorAll('.pane-row.selected').length, tabs: window.__fc.tabsInfo().filter((t) => !t.live).length }));
    need(pre.selected === 0 && pre.tabs === 0, `前置：沒有選定 pane、沒有檔案分頁（${JSON.stringify(pre)}）`);
    await switchLeftTab(ctx, '檔案');
    const shown = await ctx.cdp.poll((t) => window.__fc.filesText().includes(t), [NO_PANE_TEXT], UI_TIMEOUT_MS);
    check(!!shown, `「檔案」分頁顯示空狀態「${NO_PANE_TEXT}」（實際 #files 文字「${await ctx.cdp.run(() => window.__fc.filesText())}」）`);
    await sleep(1000);
    const fileReqs = ctx.net.since(ctx.loadedAt).filter((r) => isFileEndpoint(r.kind));
    check(fileReqs.length === 0, `頁面沒有發出任何檔案端點的請求（實際 ${JSON.stringify(fileReqs.map((r) => r.url))}）`);
  });
}

// GIVEN 選定 w1:p1（review-repo）並展開 src WHEN 改選 w2:p1（other-repo）再改選回 w1:p1 THEN src 仍展開。
async function segExpandStatePerRoot() {
  await withCockpit('tree-expand', {}, async (ctx) => {
    await openTree(ctx);
    await expandDir(ctx, 'src');
    await waitRow(ctx, 'src/main.rs');
    await selectPane(ctx, PANE_OTHER);
    const switched = await ctx.cdp.poll(() => window.__fc.filesHeaderText().includes('other-repo') && !!window.__fc.row('lib'), [], UI_TIMEOUT_MS);
    need(!!switched, `改選 ${PANE_OTHER} 後檔案樹換成 other-repo（頂端「${await ctx.cdp.run(() => window.__fc.filesHeaderText())}」）`);
    await selectPane(ctx, PANE_REVIEW);
    const back = await ctx.cdp.poll(() => window.__fc.filesHeaderText().includes('review-repo') && !!window.__fc.row('src'), [], UI_TIMEOUT_MS);
    need(!!back, '改選回 wJ:p4 後檔案樹換回 review-repo');
    await sleep(500);
    const st = await ctx.cdp.run(() => ({
      src: window.__fc.row('src') ? window.__fc.row('src').getAttribute('aria-expanded') : null,
      child: !!window.__fc.row('src/main.rs') && window.__fc.visible(window.__fc.row('src/main.rs')),
    }));
    check(st.src === 'true' && st.child, `review-repo 的 src 仍為展開，src/main.rs 可見（${JSON.stringify(st)}）`);
  });
}

// 頁面端：檔案樹「節點沒被換掉」的快照與比對（fix round 1，Codex finding 2 的同性質檢查）。不只比 tree 與
// 每一列本身，還比 tree 底下所有子孫節點（列內的 icon、名稱等被換掉也算），並確認目前的捲動容器仍是原本那一個、
// 仍在頁面上，捲動位置讀「目前的」捲動容器。finderKey 為 null 時用 window.__fc；自我測試用合成 DOM 時傳入
// 另一個提供 tree()／rows()／treeScroller() 的全域物件名稱。
function treeSnapshot(finderKey) {
  const F = finderKey ? window[finderKey] : window.__fc;
  const tree = F.tree();
  const sc = F.treeScroller();
  if (!tree || !sc) return { error: `tree ${!!tree}、捲動容器 ${!!sc}` };
  const rows = F.rows();
  window.__fcTreeKeep = {
    tree,
    sc,
    rows,
    descendants: Array.from(tree.querySelectorAll('*')),
    expanded: rows.map((r) => r.getAttribute('aria-expanded')),
    scrollTop: sc.scrollTop,
  };
  return { rows: rows.length, descendants: window.__fcTreeKeep.descendants.length, scrollTop: sc.scrollTop };
}
function treeCompare(finderKey) {
  const F = finderKey ? window[finderKey] : window.__fc;
  const k = window.__fcTreeKeep;
  const rows = F.rows();
  const tree = F.tree();
  const desc = tree ? Array.from(tree.querySelectorAll('*')) : [];
  const sc = F.treeScroller();
  return {
    sameTree: tree === k.tree && k.tree.isConnected,
    sameRows: rows.length === k.rows.length && rows.every((r, i) => r === k.rows[i]),
    sameDescendants: desc.length === k.descendants.length && desc.every((n, i) => n === k.descendants[i]),
    descendants: desc.length,
    sameExpanded: JSON.stringify(rows.map((r) => r.getAttribute('aria-expanded'))) === JSON.stringify(k.expanded),
    scConnected: k.sc.isConnected,
    scSame: sc === k.sc && k.sc.isConnected,
    scrollTop: sc ? sc.scrollTop : null,
    expectScrollTop: k.scrollTop,
  };
}
// 頁面端：以合成 DOM 驗 treeSnapshot／treeCompare 的偵測力（檔案樹由 file-review task 4.2 才實作，真的
// 檔案樹還不存在，負對照先在合成 DOM 上跑）：(a) 什麼都不動→全部相同；(b) 只把某一列的 innerHTML 重設（列本身
// 不變、列內子孫換新）→ sameDescendants 必須為 false；(c) 把整個捲動容器換成內容相同的新節點 → scSame、
// sameTree 必須為 false。
function syntheticTreeDetectorProbe() {
  const host = document.createElement('div');
  host.style.cssText = 'position:fixed;left:0;top:0;width:200px;';
  const build = () => {
    let h = '<div data-fc-sc style="height:60px;overflow:auto"><div role="tree">';
    for (let i = 0; i < 20; i++) h += `<div role="treeitem" title="f${i}.txt" aria-level="1"><img alt=""><span>f${i}.txt</span></div>`;
    return h + '</div></div>';
  };
  host.innerHTML = build();
  document.body.appendChild(host);
  window.__fcSynthTree = {
    tree: () => host.querySelector('[role="tree"]'),
    rows: () => Array.from(host.querySelectorAll('[role="treeitem"]')),
    treeScroller: () => host.querySelector('[data-fc-sc]'),
  };
  host.querySelector('[data-fc-sc]').scrollTop = 100;
  const out = {};
  treeSnapshot('__fcSynthTree');
  out.untouched = treeCompare('__fcSynthTree');
  host.querySelectorAll('[role="treeitem"]')[3].innerHTML = host.querySelectorAll('[role="treeitem"]')[3].innerHTML;
  out.rowInnerReplaced = treeCompare('__fcSynthTree');
  treeSnapshot('__fcSynthTree');
  host.innerHTML = build();
  host.querySelector('[data-fc-sc]').scrollTop = 100;
  out.scrollerReplaced = treeCompare('__fcSynthTree');
  host.remove();
  delete window.__fcSynthTree;
  return out;
}

// GIVEN 已展開 src 並往下捲動、焦點在某個檔案列上，投影每 100 ms 推送 WHEN 經過 3 秒 THEN 檔案樹 DOM
// 節點沒被換掉，展開狀態、捲動位置與焦點都不變。為了真的能捲動，暫存副本加一個含 80 個檔案的 many/。
async function segRepaintKeepsTree() {
  await withCockpit(
    'tree-repaint',
    {
      env: { COCKPIT_PREVIEW_PUSH_MS: '100' },
      beforeLoad: (preview) => {
        for (let i = 1; i <= 80; i++) writeTemp(preview, `many/f-${String(i).padStart(3, '0')}.txt`, `file ${i}\n`);
      },
    },
    async (ctx) => {
      await openTree(ctx);
      await expandDir(ctx, 'src');
      await expandDir(ctx, 'many');
      await waitRow(ctx, 'many/f-080.txt');
      const setup = await ctx.cdp.run(() => {
        const sc = window.__fc.treeScroller();
        if (!sc) return { error: '找不到檔案樹的捲動容器（契約 C2）' };
        sc.scrollTop = Math.round((sc.scrollHeight - sc.clientHeight) / 2);
        const box = sc.getBoundingClientRect();
        const rows = window.__fc.rows();
        const target = rows.find((r) => {
          const b = r.getBoundingClientRect();
          return !r.hasAttribute('aria-expanded') && b.top >= box.top + 20 && b.bottom <= box.bottom - 20;
        });
        if (!target) return { error: '捲動後找不到完整可見的檔案列' };
        target.focus({ preventScroll: true });
        window.__fcFocusTarget = target;
        return { scrollTop: sc.scrollTop, max: sc.scrollHeight - sc.clientHeight, focus: target.getAttribute('title'), focused: document.activeElement === target };
      });
      need(!setup.error && setup.scrollTop > 0 && setup.focused, `前置：檔案樹捲到中段、焦點在檔案列上（${JSON.stringify(setup)}）`);
      const snap = await ctx.cdp.run(treeSnapshot, null);
      need(!snap.error, `前置：記下檔案樹、每一列與列內所有子孫節點、捲動容器（${JSON.stringify(snap)}）`);
      const v0 = await ctx.cdp.run(() => document.getElementById('version').getAttribute('data-state-version'));
      let repaints = 0;
      let lastV = v0;
      const start = Date.now();
      while (Date.now() - start < 3000) {
        await sleep(100);
        const v = await ctx.cdp.run(() => document.getElementById('version').getAttribute('data-state-version'));
        if (v !== lastV) {
          repaints += 1;
          lastV = v;
        }
      }
      check(repaints >= 10, `3 秒內真的發生了多次整頁重畫（#version 的 data-state-version 變化 ${repaints} 次）`);
      const after = await ctx.cdp.run(treeCompare, null);
      const focus = await ctx.cdp.run(() => ({
        same: document.activeElement === window.__fcFocusTarget,
        active: document.activeElement ? document.activeElement.getAttribute('title') || document.activeElement.tagName : null,
      }));
      check(
        after.sameTree && after.sameRows && after.sameDescendants,
        `檔案樹的 DOM 節點沒有被換掉（tree ${after.sameTree}、列 ${after.sameRows}、列內子孫 ${after.sameDescendants}；${after.descendants} 個節點）`
      );
      check(after.sameExpanded, '展開狀態不變');
      check(after.scSame, `捲動容器仍是原本那一個且仍在頁面上（${JSON.stringify({ scSame: after.scSame, scConnected: after.scConnected })}）`);
      check(after.scrollTop !== null && Math.abs(after.scrollTop - after.expectScrollTop) <= 1, `捲動位置不變（讀目前的捲動容器：${after.expectScrollTop} → ${after.scrollTop}）`);
      check(focus.same, `焦點仍在原本的檔案列上（${setup.focus}；目前 ${focus.active}）`);
    }
  );
}

// GIVEN 選定 w1:p1（cwd＝review-repo/src）、左欄在「檔案」分頁、檔案樹顯示 review-repo WHEN 同一個 pane 的
// cwd 改成 other-repo（pane id 不變；ui_preview 的 COCKPIT_PREVIEW_CWD_TO_OTHER_REPO，推送 300 ms）THEN 不必手動
// 重新整理或切換分頁，檔案樹就換成 other-repo 的第一層；而且根目錄只因 cwd 改變查了一次，不是每份投影都查
// （file-review 最終修正波 F1；spec「左欄檔案樹」：根目錄的子項目在根目錄改變時讀取）。
const CWD_MOVE_AFTER_MS = 7000;
async function segCwdChangeRehomesTree() {
  await withCockpit(
    'tree-cwd',
    { env: { COCKPIT_PREVIEW_PUSH_MS: '300', COCKPIT_PREVIEW_CWD_TO_OTHER_REPO: `${PANE_REVIEW}=${CWD_MOVE_AFTER_MS}` } },
    async (ctx) => {
      const paneState = async () => {
        const st = await apiJson(ctx, '/api/state');
        if (st.status !== 200 || !st.body) need(false, `/api/state 回 200（實際 ${st.status}）`);
        let cwd;
        for (const rt of st.body.runtimes) {
          if (rt.id !== RUNTIME) continue;
          for (const ws of rt.workspaces) for (const tab of ws.tabs) for (const p of tab.panes) if (p.id === PANE_REVIEW) cwd = p.cwd;
        }
        return { cwd, version: st.body.version };
      };
      const other = await rootInfo(ctx, PANE_OTHER);
      const otherList = await apiJson(ctx, `/api/files/${RUNTIME}/${other.root_id}/list`);
      need(otherList.status === 200, '前置：other-repo 列目錄端點回 200');
      const expected = otherList.body.entries.map((e) => e.name).sort();
      await openTree(ctx);
      const before = await paneState();
      need(before.cwd === path.join(ctx.preview.reviewRepo, 'src'), `前置：${PANE_REVIEW} 的 cwd 還是 review-repo/src（實際 ${before.cwd}）`);
      const header0 = await ctx.cdp.run(() => window.__fc.filesHeaderText());
      need(header0.includes('review-repo') && !header0.includes('other-repo'), `前置：檔案樹頂端是 review-repo（實際「${header0}」）`);
      const tMark = Date.now();
      let movedAt = null;
      while (Date.now() - tMark < CWD_MOVE_AFTER_MS + 5000) {
        if ((await paneState()).cwd === ctx.preview.otherRepo) {
          movedAt = Date.now();
          break;
        }
        await sleep(100);
      }
      need(movedAt !== null, `前置：服務端投影中 ${PANE_REVIEW} 的 cwd 改成 other-repo`);
      const rehomed = await ctx.cdp.poll(() => window.__fc.filesHeaderText().includes('other-repo') && !!window.__fc.row('lib'), [], UI_TIMEOUT_MS);
      const header1 = await ctx.cdp.run(() => window.__fc.filesHeaderText());
      check(!!rehomed, `cwd 改變後 ${UI_TIMEOUT_MS} ms 內檔案樹換成 other-repo，不需重新整理或切換分頁（${Date.now() - movedAt} ms；頂端「${header1}」）`);
      await sleep(3000);
      const rows = await ctx.cdp.run(() => window.__fc.rowsInfo());
      const top = rows.filter((r) => r.title && !r.title.includes('/')).map((r) => r.title).sort();
      check(JSON.stringify(top) === JSON.stringify(expected), `第一層列＝other-repo 的子項目（預期 ${JSON.stringify(expected)}，實際 ${JSON.stringify(top)}）`);
      const after = await paneState();
      const pushed = after.version - before.version;
      need(pushed >= 10, `前置：觀察期間服務推送了至少 10 份投影（實際 ${pushed}）`);
      const roots = ctx.net.since(tMark, 'root');
      check(roots.length === 1, `觀察期間（${pushed} 份投影）根目錄查詢恰 1 次——只在 cwd 改變時查，不是每份投影都查（實際 ${roots.length}）`);
    }
  );
}

// ---------------------------------------------------------------------------
// file-review：檔案分頁
// ---------------------------------------------------------------------------

// GIVEN 下半部只有 Live Output 分頁 WHEN 在檔案樹點 README.md THEN 出現 README.md 分頁並成為目前分頁，
// Live Output 仍在第一個。
async function segOpenAddsTab() {
  await withCockpit('tab-open', {}, async (ctx) => {
    await openTree(ctx);
    const before = await tabsInfo(ctx);
    need(before.length === 1 && before[0].live, `前置：下半部只有 Live Output 分頁（契約 C3）；實際 ${JSON.stringify(before)}；目前 DOM：${await contractDump(ctx)}`);
    await openFile(ctx, 'README.md');
    const after = await tabsInfo(ctx);
    check(after.length === 2, `分頁數 1 → 2（實際 ${after.length}）`);
    check(after[0] && after[0].live, 'Live Output 分頁仍在第一個');
    check(after[1] && after[1].path === 'README.md' && after[1].selected && after[1].label.includes('README.md'), `README.md 分頁在第二個、顯示檔名、為目前分頁（${JSON.stringify(after[1])}）`);
    check(after[1] && typeof after[1].title === 'string' && after[1].title.includes('README.md') && after[1].title.includes('review-repo'), `分頁 title 含相對路徑與根目錄名稱（${after[1] && after[1].title}）`);
  });
}

// GIVEN 已打開 README.md 與 docs/a.md，目前為 docs/a.md WHEN 再點 README.md THEN 分頁數不變，目前改為 README.md。
async function segReopenNoNewTab() {
  await withCockpit('tab-reopen', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    await openFile(ctx, 'docs/a.md');
    const before = await tabsInfo(ctx);
    need(before.length === 3 && before[2].path === 'docs/a.md' && before[2].selected, `前置：Live Output、README.md、docs/a.md 三個分頁，目前 docs/a.md（${JSON.stringify(before)}）`);
    need(await ctx.cdp.clickEl((p) => window.__fc.row(p), ['README.md'], '檔案列 README.md'), '再點檔案樹的 README.md');
    await ctx.cdp.poll(() => !!window.__fc.fileTab('README.md') && window.__fc.fileTab('README.md').getAttribute('aria-selected') === 'true', [], UI_TIMEOUT_MS);
    await sleep(300);
    const after = await tabsInfo(ctx);
    check(after.length === before.length, `分頁數不變（${before.length} → ${after.length}）`);
    const cur = after.find((t) => t.selected);
    check(!!cur && cur.path === 'README.md', `目前分頁改為 README.md（實際 ${JSON.stringify(cur)}）`);
  });
}

// GIVEN 分頁依序為 Live Output、a.md、b.md、c.md，目前為 b.md WHEN 關閉 b.md THEN 目前分頁為 c.md。
async function segCloseCurrentTab() {
  await withCockpit(
    'tab-close',
    {
      beforeLoad: (preview) => {
        for (const n of ['a', 'b', 'c']) writeTemp(preview, `${n}.md`, `# ${n}.md\n\n${n} 的內容（files-check）。\n`);
      },
    },
    async (ctx) => {
      await openTree(ctx);
      for (const n of ['a.md', 'b.md', 'c.md']) await openFile(ctx, n);
      await clickTab(ctx, 'b.md');
      const before = await tabsInfo(ctx);
      need(JSON.stringify(before.map((t) => (t.live ? 'LIVE' : t.path))) === JSON.stringify(['LIVE', 'a.md', 'b.md', 'c.md']), `前置：分頁依序為 Live Output、a.md、b.md、c.md（${JSON.stringify(before)}）`);
      const hasClose = await ctx.cdp.run(() => !!window.__fc.closeButtonFor(window.__fc.fileTab('b.md')));
      need(hasClose, `b.md 分頁有關閉按鈕（契約 C4：aria-label 以「關閉」開頭的 button）`);
      need(await ctx.cdp.clickEl(() => window.__fc.closeButtonFor(window.__fc.fileTab('b.md')), [], 'b.md 的關閉按鈕'), '點 b.md 的關閉按鈕');
      const gone = await ctx.cdp.poll(() => !window.__fc.fileTab('b.md'), [], UI_TIMEOUT_MS);
      check(!!gone, 'b.md 分頁已關閉');
      const after = await tabsInfo(ctx);
      const cur = after.find((t) => t.selected);
      check(!!cur && cur.path === 'c.md', `目前分頁為 c.md（實際 ${JSON.stringify(cur)}；分頁 ${JSON.stringify(after.map((t) => t.path || 'LIVE'))}）`);
    }
  );
}

// GIVEN 已打開 README.md 分頁 WHEN 點左欄 Project 分頁中的另一個 Project THEN README.md 分頁仍在、仍為目前分頁。
async function segProjectSwitchKeepsTabs() {
  await withCockpit('tab-project', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    await switchLeftTab(ctx, 'Project');
    const other = await ctx.cdp.poll(() => {
      const items = Array.from(document.querySelectorAll('button[data-action="select-project"]')).filter((b) => window.__fc.visible(b));
      const o = items.find((b) => b.getAttribute('aria-current') !== 'true');
      return o ? o.getAttribute('data-project') : null;
    }, [], UI_TIMEOUT_MS);
    need(!!other, '左欄 Project 分頁中有另一個可見的 Project 項目');
    const sel = `button[data-action="select-project"][data-project="${other}"]`;
    need(await ctx.cdp.clickEl((s) => document.querySelector(s), [sel], `Project ${other}`), `點 Project ${other}`);
    const switched = await ctx.cdp.poll((s) => {
      const b = document.querySelector(s);
      return !!b && b.getAttribute('aria-current') === 'true';
    }, [sel], UI_TIMEOUT_MS);
    need(!!switched, `Project ${other} 成為目前 Project`);
    await sleep(300);
    const after = await tabsInfo(ctx);
    const readme = after.find((t) => t.path === 'README.md');
    check(!!readme, 'README.md 分頁仍在');
    check(!!readme && readme.selected, 'README.md 仍為目前分頁');
  });
}

// ---------------------------------------------------------------------------
// file-review：檔案檢視器
// ---------------------------------------------------------------------------

// GIVEN README.md 含 [設計](docs/design.md#決策) WHEN 在其分頁點這個連結 THEN 開啟 docs/design.md 分頁並捲到「決策」。
async function segMdRelativeLink() {
  await withCockpit('viewer-mdlink', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    const link = await ctx.cdp.poll(() => {
      const p = window.__fc.currentPanel();
      return !!p && Array.from(p.querySelectorAll('a')).some((a) => window.__fc.txt(a) === '設計');
    }, [], UI_TIMEOUT_MS);
    need(!!link, `README.md 分頁內容出現連結「設計」（契約 C3／C6）；目前分頁文字「${(await panelText(ctx)).slice(0, 120)}」`);
    const href0 = await ctx.cdp.run(() => location.href);
    need(
      await ctx.cdp.clickEl(() => Array.from(window.__fc.currentPanel().querySelectorAll('a')).find((a) => window.__fc.txt(a) === '設計'), [], '連結「設計」'),
      '點 README.md 內的連結「設計」'
    );
    const opened = await ctx.cdp.poll(() => !!window.__fc.fileTab('docs/design.md') && window.__fc.fileTab('docs/design.md').getAttribute('aria-selected') === 'true', [], UI_TIMEOUT_MS);
    check(!!opened, 'docs/design.md 分頁開啟並成為目前分頁');
    const scrolled = await ctx.cdp.poll(() => {
      const p = window.__fc.currentPanel();
      if (!p) return false;
      const h = Array.from(p.querySelectorAll('h1,h2,h3,h4,h5,h6')).find((x) => window.__fc.txt(x) === '決策');
      const sc = window.__fc.contentScroller();
      if (!h || !sc) return false;
      const hb = h.getBoundingClientRect();
      const sb = sc.getBoundingClientRect();
      return sc.scrollTop > 0 && hb.top >= sb.top - 2 && hb.top < sb.bottom - 10 ? { scrollTop: sc.scrollTop, headingTop: hb.top - sb.top } : false;
    }, [], UI_TIMEOUT_MS);
    check(!!scrolled, `捲動到「決策」標題（標題在內容捲動容器的可視範圍內且 scrollTop > 0；${JSON.stringify(scrolled)}）`);
    check((await ctx.cdp.run(() => location.href)) === href0, '點連結沒有讓整頁離開 Cockpit');
  });
}

// GIVEN README.md 含 ![logo](https://example.com/logo.png) 與 ![圖](docs/pic.png) WHEN 開啟其分頁 THEN 沒有對
// example.com 的任何請求、該處顯示替代文字 logo；docs/pic.png 經原始內容端點載入（且成功）。
async function segExternalImageBlocked() {
  await withCockpit('viewer-img', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    need(!!(await waitPanelText(ctx, 'review-repo')), `README.md 分頁顯示渲染後的內容（契約 C6）；目前分頁文字「${(await panelText(ctx)).slice(0, 120)}」`);
    await sleep(1500);
    const ext = ctx.net.reqs.filter((r) => {
      try {
        return new URL(r.url).hostname.endsWith('example.com');
      } catch {
        return false;
      }
    });
    check(ext.length === 0, `頁面沒有對 example.com 發出任何請求（實際 ${JSON.stringify(ext.map((r) => r.url))}）`);
    const dom = await ctx.cdp.run(() => {
      const v = window.__fc.viewer() || window.__fc.currentPanel();
      const imgs = Array.from(v.querySelectorAll('img'));
      const pic = imgs.find((i) => (i.currentSrc || i.src).includes('/raw/docs/pic.png'));
      return {
        extImg: imgs.filter((i) => (i.getAttribute('src') || '').includes('example.com')).length,
        altShown: window.__fc.txt(v).includes('logo'),
        pic: pic ? { src: pic.currentSrc || pic.src, complete: pic.complete, w: pic.naturalWidth } : null,
      };
    });
    check(dom.extImg === 0 && dom.altShown, `外部圖片的位置顯示替代文字 logo、沒有指向 example.com 的 img（${JSON.stringify(dom)}）`);
    const picReq = ctx.net.reqs.filter((r) => r.kind === 'raw' && new URL(r.url).pathname.endsWith('/raw/docs/pic.png'));
    check(picReq.some((r) => r.status === 200), `docs/pic.png 經原始內容端點載入且成功（請求 ${JSON.stringify(picReq.map((r) => [r.url, r.status]))}）`);
    check(!!dom.pic && dom.pic.complete && dom.pic.w > 0, `內容中的 docs/pic.png 圖片已載入（${JSON.stringify(dom.pic)}）`);
  });
}

// GIVEN page.html 含 <script>parent.postMessage('ran','*')</script> 與同目錄 style.css WHEN 開啟其分頁 THEN
// 沒有收到 ran；iframe sandbox 不含 allow-scripts（也不含 allow-same-origin，見檢視器需求）；style.css 有被請求且成功。
async function segHtmlScriptBlocked() {
  await withCockpit('viewer-html', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'page.html');
    const hasFrame = await ctx.cdp.poll(() => !!window.__fc.currentPanel() && !!window.__fc.currentPanel().querySelector('iframe'), [], UI_TIMEOUT_MS);
    need(!!hasFrame, `page.html 分頁內有 iframe（契約 C6）；目前 DOM：${await contractDump(ctx)}`);
    const t = Date.now();
    while (Date.now() - t < UI_TIMEOUT_MS && !ctx.net.reqs.some((r) => r.kind === 'raw' && r.url.endsWith('/raw/style.css') && r.status !== null)) await sleep(100);
    await sleep(1500);
    const st = await ctx.cdp.run(() => {
      const f = window.__fc.currentPanel().querySelector('iframe');
      return { has: f.hasAttribute('sandbox'), tokens: Array.from(f.sandbox), src: f.getAttribute('src'), messages: window.__fcMessages.slice() };
    });
    log(`iframe：${JSON.stringify(st)}`);
    check(!st.messages.includes('ran'), `頁面沒有收到 ran 訊息（收到 ${JSON.stringify(st.messages)}）`);
    check(st.has && !st.tokens.includes('allow-scripts') && !st.tokens.includes('allow-same-origin'), `iframe 帶 sandbox 屬性且不含 allow-scripts／allow-same-origin（${JSON.stringify(st.tokens)}）`);
    const css = ctx.net.reqs.filter((r) => r.kind === 'raw' && new URL(r.url).pathname.endsWith('/raw/style.css'));
    check(css.some((r) => r.status === 200), `style.css 有被請求且成功（${JSON.stringify(css.map((r) => [r.url, r.status, r.session ? 'oopif' : 'page']))}）`);
  });
}

// GIVEN nometa.html 以 UTF-8 寫成、含「檔案瀏覽」，沒有 <meta charset> 也沒有 BOM WHEN 開啟其分頁 THEN 原始內容回應的
// Content-Type 為 text/html; charset=utf-8；iframe 內文件含「檔案瀏覽」（不是 Big5 等舊編碼解出的亂碼）。
// iframe 帶 sandbox 且不含 allow-scripts／allow-same-origin，頁面讀不到它的 contentDocument、也不能在裡面跑腳本，
// 所以用 CDP DOM 讀解析後的文件：同行程 frame 由頁面 session 的 pierce 取得，OOPIF 則在它的子 session 讀。
async function segUtf8HtmlWithoutMeta() {
  const rel = 'nometa.html';
  const needle = '檔案瀏覽';
  await withCockpit('viewer-html-charset', { beforeLoad: (preview) => writeTemp(preview, rel, Buffer.from(`<h1>${needle}</h1><p>files-check 沒有宣告編碼的 UTF-8 HTML。</p>`, 'utf8')) }, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, rel);
    const hasFrame = await ctx.cdp.poll(() => !!window.__fc.currentPanel() && !!window.__fc.currentPanel().querySelector('iframe'), [], UI_TIMEOUT_MS);
    need(!!hasFrame, `${rel} 分頁內有 iframe（契約 C6）；目前 DOM：${await contractDump(ctx)}`);
    const isNometaRaw = (r) => r.kind === 'raw' && new URL(r.url).pathname.endsWith(`/raw/${rel}`);
    const t = Date.now();
    while (Date.now() - t < UI_TIMEOUT_MS && !ctx.net.reqs.some((r) => isNometaRaw(r) && r.status !== null)) await sleep(100);
    const raws = ctx.net.reqs.filter((r) => isNometaRaw(r) && r.status !== null);
    need(raws.length > 0, `${rel} 的原始內容有被請求（${JSON.stringify(ctx.net.reqs.filter((r) => r.kind === 'raw').map((r) => r.url))}）`);
    const types = raws.map((r) => (r.headers || {})['content-type']);
    check(raws.every((r) => r.status === 200) && types.every((v) => v === 'text/html; charset=utf-8'), `原始內容回應 200 且 Content-Type 為 text/html; charset=utf-8（${JSON.stringify(raws.map((r) => [r.status, (r.headers || {})['content-type']]))}）`);

    // 在一個 session 的 DOM 樹裡找 documentURL 指向 raw/nometa.html 的文件，回傳其 outerHTML（找不到回 null）。
    const frameHtml = async (sid) => {
      const doc = await ctx.cdp.send('DOM.getDocument', { depth: -1, pierce: true }, sid);
      if (!doc.result) return null;
      const stack = [doc.result.root];
      while (stack.length) {
        const n = stack.pop();
        if (n.nodeType === 9 && n.documentURL && n.documentURL.split('?')[0].endsWith(`/raw/${rel}`)) {
          const html = await ctx.cdp.send('DOM.getOuterHTML', { nodeId: n.nodeId }, sid);
          return html.result ? html.result.outerHTML : null;
        }
        if (n.contentDocument) stack.push(n.contentDocument);
        for (const c of n.children || []) stack.push(c);
        for (const c of n.shadowRoots || []) stack.push(c);
      }
      return null;
    };
    // 每輪重算：OOPIF 的子 session 可能在 iframe 導航後才 attach；文件導航請求記在父 session，不能靠它找子 session。
    const sessionIds = () => [null, ...new Set(ctx.net.sessions.filter((s) => s.type === 'iframe').map((s) => s.sid))];
    let sessions = sessionIds();
    let html = null;
    let where = null;
    const deadline = Date.now() + UI_TIMEOUT_MS;
    while (Date.now() < deadline) {
      sessions = sessionIds();
      for (const sid of sessions) {
        html = await frameHtml(sid);
        if (html && html.includes('files-check')) {
          where = sid ? 'oopif' : 'page';
          break;
        }
      }
      if (where) break;
      await sleep(200);
    }
    need(html !== null, `讀得到 iframe 內 ${rel} 的文件（CDP DOM；session：${JSON.stringify(sessions)}）`);
    log(`iframe 文件（${where || '未載入完成'}）：${(html || '').slice(0, 200)}`);
    check(html.includes(needle), `iframe 內文件含「${needle}」，不是亂碼（實際前 200 字「${html.slice(0, 200)}」）`);
  });
}

// GIVEN report.pdf 共 3 頁、第 1 頁含中文標題 WHEN 開啟其分頁 THEN 工具列「1 / 3」，3 頁都畫出內容，第 1 頁中文標題
// 可辨識。像素判準見 PDF_* 常數；「非方框」由截圖目視（路徑印在輸出）。
async function segChinesePdf() {
  await withCockpit('viewer-pdf', {}, async (ctx) => {
    try {
      await openTree(ctx);
      await openFile(ctx, 'report.pdf');
      const counter = await ctx.cdp.poll(() => /(^|[^0-9])1\s*\/\s*3([^0-9]|$)/.test(window.__fc.panelText()), [], 10000);
      check(!!counter, `工具列顯示「1 / 3」（目前分頁文字「${(await panelText(ctx)).slice(0, 160)}」）`);
      const canvases = await ctx.cdp.poll(() => {
        const p = window.__fc.currentPanel();
        return p && p.querySelectorAll('canvas').length >= 3 ? p.querySelectorAll('canvas').length : false;
      }, [], 10000);
      need(!!canvases, `PDF 分頁內每頁一個 canvas、共至少 3 個（契約 C6）；目前 DOM：${await contractDump(ctx)}`);
      for (let i = 0; i < 3; i++) {
        const ink = await ctx.cdp.poll(
          (idx, min) => {
            const c = window.__fc.currentPanel().querySelectorAll('canvas')[idx];
            c.scrollIntoView({ block: 'center' });
            const s = window.__fc.inkStats(c, null);
            return s.ratio > min ? s : false;
          },
          [i, PDF_PAGE_INK_MIN],
          8000,
          250
        );
        const last = ink || (await ctx.cdp.run((idx) => window.__fc.inkStats(window.__fc.currentPanel().querySelectorAll('canvas')[idx], null), i));
        check(!!ink, `第 ${i + 1} 頁畫出內容（整頁非背景比例 > ${PDF_PAGE_INK_MIN}；實際 ${JSON.stringify(last)}）`);
      }
      const title = await ctx.cdp.run(
        (band) => {
          const c = window.__fc.currentPanel().querySelectorAll('canvas')[0];
          c.scrollIntoView({ block: 'start' });
          return window.__fc.inkStats(c, band);
        },
        PDF_TITLE_BAND
      );
      check(title.ratio > PDF_TITLE_INK_MIN, `第 1 頁中文標題帶（頁高 ${PDF_TITLE_BAND.y0}–${PDF_TITLE_BAND.y1}）非背景比例 ${title.ratio} > ${PDF_TITLE_INK_MIN}（空白頁為 0）`);
    } finally {
      // fix round 1（Codex finding 1，控制端裁決 R22）：「非方框」由控制端目視截圖判定，本段的 PASS 不含這一半；
      // 截圖存不下來就沒有東西可目視，判 FAIL。
      const shot = await saveScreenshot(ctx, 'chinese-pdf');
      check(shot.ok, `viewport 截圖已存檔，可供目視（${shot.ok ? shot.file : shot.error}）`);
      if (shot.ok) console.log(`需目視確認（非方框）：${shot.file}`);
    }
  });
}

// 回傳 { ok, file } 或 { ok: false, error }；檔案必須真的寫出且非空才算成功。
async function saveScreenshot(ctx, name) {
  try {
    fs.mkdirSync(SCRATCH, { recursive: true });
    const shot = await ctx.cdp.send('Page.captureScreenshot', { format: 'png' });
    if (!shot.result || !shot.result.data) return { ok: false, error: `Page.captureScreenshot 沒有回傳影像（${JSON.stringify(shot.error || null)}）` };
    const file = path.join(SCRATCH, `files-check-${name}-${new Date().toISOString().replace(/[:.]/g, '-')}.png`);
    fs.writeFileSync(file, Buffer.from(shot.result.data, 'base64'));
    if (!(fs.existsSync(file) && fs.statSync(file).size > 0)) return { ok: false, error: `截圖檔不存在或為空：${file}` };
    return { ok: true, file };
  } catch (e) {
    return { ok: false, error: `截圖失敗：${e.message}` };
  }
}

// GIVEN note.txt 含 <b>x</b> WHEN 開啟其分頁 THEN 字樣原樣顯示，內容區沒有 b 元素。
async function segPlainTextNotParsed() {
  await withCockpit('viewer-text', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'note.txt');
    const shown = await waitPanelText(ctx, '<b>x</b>');
    check(!!shown, `字樣 <b>x</b> 原樣顯示（目前分頁文字「${(await panelText(ctx)).slice(0, 160)}」）`);
    const bCount = await ctx.cdp.run(() => (window.__fc.currentPanel() ? window.__fc.currentPanel().querySelectorAll('b').length : -1));
    check(bCount === 0, `內容區沒有 b 元素（實際 ${bCount}；-1＝沒有目前 tabpanel）`);
  });
}

// ---------------------------------------------------------------------------
// file-review：自動更新
// ---------------------------------------------------------------------------

// GIVEN long.md 分頁為目前分頁、已捲到中段 WHEN 在檔案末端加一段文字 THEN 3 秒內新段落出現，捲動位置不變。
async function segAutoUpdateKeepsScroll() {
  await withCockpit('auto-scroll', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'long.md');
    // 等的是檢視器（契約 C6 的 [data-viewer]）內的文字，不是整個 tabpanel：tabpanel 的工具列本來就顯示
    // 相對路徑「long.md」，比內容早出現，比對到它就去量捲動容器時內容還沒畫進去（file-review task 4.6 查明）。
    need(!!(await waitViewerText(ctx, 'long.md')), 'long.md 分頁的檢視器顯示內容');
    const s0 = await ctx.cdp.run(() => {
      const sc = window.__fc.contentScroller();
      if (!sc) return null;
      sc.scrollTop = Math.round((sc.scrollHeight - sc.clientHeight) / 2);
      return { scrollTop: sc.scrollTop, max: sc.scrollHeight - sc.clientHeight };
    });
    need(!!s0 && s0.scrollTop > 0, `前置：內容可捲動並捲到中段（契約 C6；${JSON.stringify(s0)}）`);
    await sleep(600);
    const before = await ctx.cdp.run(() => window.__fc.contentScroller().scrollTop);
    const marker = `FILES-CHECK-APPEND-${Date.now()}`;
    fs.appendFileSync(path.join(ctx.preview.reviewRepo, 'long.md'), `\n\n## 追加段落\n\n${marker} 新段落。\n`);
    const t0 = Date.now();
    const shown = await waitPanelText(ctx, marker, 3000);
    const elapsed = Date.now() - t0;
    check(!!shown, `3 秒內新段落出現在內容中（${shown ? `${elapsed} ms` : '逾時'}）`);
    const after = await ctx.cdp.run(() => (window.__fc.contentScroller() ? window.__fc.contentScroller().scrollTop : null));
    check(after !== null && Math.abs(after - before) <= 1, `捲動位置不變（${before} → ${after}）`);
  });
}

// GIVEN 已打開兩個檔案分頁，目前分頁是 Live Output WHEN 觀察 10 秒 THEN 服務沒有收到任何中繼資料查詢。
// 前置另外確認「檔案分頁為目前分頁時確實會查詢」，證明計數器量得到中繼資料請求。
async function segNoMetaOnLiveTab() {
  await withCockpit('auto-live', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    await openFile(ctx, 'docs/a.md');
    const tFile = Date.now();
    await sleep(2500);
    const pre = (await pageRequests(ctx)).recs.filter((r) => classify(r.url) === 'meta' && r.at && r.at.tab === 'docs/a.md');
    need(pre.length >= 1, `前置：檔案分頁 docs/a.md 為目前分頁時會定時查詢中繼資料（頁面端記錄 ${pre.length} 次，CDP ${ctx.net.since(tFile, 'meta').length} 次）`);
    await clickTab(ctx, 'LIVE');
    await sleep(10000);
    // fix round 2（控制端裁決 R23）：每個中繼資料請求（fetch 與 XHR）在發出當下同步記下 Live Output 是否為目前
    // 分頁與目前分頁身分；Live Output 為目前分頁時發出的必須為 0。CDP 只做總數健全檢查（此時應該沒有中繼資料輪詢）。
    const recs = (await pageRequests(ctx)).recs.filter((r) => classify(r.url) === 'meta');
    const bad = recs.filter((r) => r.at && r.at.live === 'true');
    check(bad.length === 0, `Live Output 為目前分頁期間沒有發出任何中繼資料查詢（實際 ${bad.length} 次：${JSON.stringify(bad.map((r) => `${r.via} ${r.url}`))}）`);
    const sanity = await cdpPageCountSanity(ctx, 'meta');
    check(sanity.ok, `CDP 總數健全檢查：CDP 看到的中繼資料請求數等於頁面端記錄數（${JSON.stringify(sanity)}）`);
  });
}

// GIVEN plan.md 分頁為目前分頁 WHEN 檔案被刪除，5 秒後以新內容重新建立 THEN 刪除期間內容標為過期並顯示
// 「檔案已不存在」；重新建立後 3 秒內顯示新內容、過期標示消失。
async function segDeletedThenRecreated() {
  await withCockpit(
    'auto-delete',
    { beforeLoad: (preview) => writeTemp(preview, 'plan.md', '# plan.md\n\nPLAN-OLD 舊內容。\n') },
    async (ctx) => {
      await openTree(ctx);
      await openFile(ctx, 'plan.md');
      need(!!(await waitPanelText(ctx, 'PLAN-OLD')), 'plan.md 分頁顯示舊內容');
      const file = path.join(ctx.preview.reviewRepo, 'plan.md');
      fs.rmSync(file);
      const tDel = Date.now();
      const stale = await ctx.cdp.poll(
        (a, b) => {
          const t = window.__fc.panelText();
          return t.includes(a) && t.includes(b) && t.includes('PLAN-OLD');
        },
        [STALE_TEXT, GONE_FILE_TEXT],
        4800
      );
      check(!!stale, `刪除期間：保留舊內容並標為過期（可見文字「${STALE_TEXT}」，契約 C7），顯示「${GONE_FILE_TEXT}」（目前分頁文字「${(await panelText(ctx)).slice(0, 160)}」）`);
      const wait = 5000 - (Date.now() - tDel);
      if (wait > 0) await sleep(wait);
      fs.writeFileSync(file, '# plan.md\n\nPLAN-NEW 新內容。\n');
      const t0 = Date.now();
      const fresh = await ctx.cdp.poll(
        (a, b) => {
          const t = window.__fc.panelText();
          return t.includes('PLAN-NEW') && !t.includes(a) && !t.includes(b);
        },
        [STALE_TEXT, GONE_FILE_TEXT],
        3000
      );
      check(!!fresh, `重新建立後 3 秒內顯示新內容、過期標示與「${GONE_FILE_TEXT}」消失（${fresh ? `${Date.now() - t0} ms` : `逾時；目前分頁文字「${(await panelText(ctx)).slice(0, 160)}」`}）`);
    }
  );
}

// ---------------------------------------------------------------------------
// file-review：分頁還原
// ---------------------------------------------------------------------------

// GIVEN 已打開 README.md 與 docs/a.md，目前 docs/a.md，左欄為「檔案」WHEN 重新整理 THEN 兩個分頁依原順序還原，
// 目前分頁為 docs/a.md 且顯示其內容，左欄為「檔案」。
async function segRestoreAfterReload() {
  await withCockpit('restore-reload', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    await openFile(ctx, 'docs/a.md');
    need((await ctx.cdp.run(() => window.__fc.leftSelected())) === '檔案', '前置：左欄目前為「檔案」');
    await sleep(500);
    await ctx.cdp.send('Page.reload', { ignoreCache: false });
    await waitForFirstProjection(ctx.cdp, ctx.preview.port, '重新整理後首份投影畫完');
    const restored = await ctx.cdp.poll(() => window.__fc.tabsInfo().filter((t) => !t.live).length >= 2, [], UI_TIMEOUT_MS);
    const tabs = await tabsInfo(ctx);
    check(!!restored && JSON.stringify(tabs.map((t) => (t.live ? 'LIVE' : t.path))) === JSON.stringify(['LIVE', 'README.md', 'docs/a.md']), `兩個分頁依原順序還原（實際 ${JSON.stringify(tabs)}）`);
    const cur = tabs.find((t) => t.selected);
    check(!!cur && cur.path === 'docs/a.md', `目前分頁為 docs/a.md（實際 ${JSON.stringify(cur)}）`);
    check(!!(await waitPanelText(ctx, '一個很短的 Markdown 檔案')), '目前分頁顯示 docs/a.md 的內容');
    check((await ctx.cdp.run(() => window.__fc.leftSelected())) === '檔案', '左欄為「檔案」分頁');
  });
}

// GIVEN 瀏覽器本機儲存中對應的值不是合法 JSON WHEN 載入頁面 THEN 下半部只有 Live Output 分頁，頁面其餘部分正常，
// console 有警告。鍵名不在 spec 裡（契約 C8）：先讓前端寫入，再把當下所有 localStorage 值改成 `{not json`。
async function segCorruptStorage() {
  await withCockpit('restore-corrupt', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    const keys = await ctx.cdp.poll(() => (localStorage.length > 0 ? Object.keys(localStorage) : false), [], 3000);
    need(!!keys, '前置：前端已把分頁寫進 localStorage（才有「對應的值」可以弄壞）');
    await ctx.cdp.run((ks) => {
      for (const k of ks) localStorage.setItem(k, '{not json');
      return true;
    }, keys);
    log(`已把 localStorage 的 ${JSON.stringify(keys)} 改成 {not json`);
    const tReload = Date.now();
    await ctx.cdp.send('Page.reload', { ignoreCache: false });
    await waitForFirstProjection(ctx.cdp, ctx.preview.port, '載入頁面後首份投影畫完（頁面其餘部分正常的前提）');
    await sleep(1500);
    const st = await ctx.cdp.run(() => ({
      tabs: window.__fc.tabsInfo(),
      panes: document.querySelectorAll('.pane-row').length,
      floor: !!document.querySelector('[data-region="floor"] .projects') && document.querySelector('[data-region="floor"] .projects').children.length > 0,
      leftTabs: !!window.__fc.leftTab('Project') && !!window.__fc.leftTab('檔案'),
      outputVisible: window.__fc.visible(document.getElementById('output')),
    }));
    check(st.tabs.length === 1 && st.tabs[0].live && st.tabs[0].selected, `下半部只有 Live Output 分頁且為目前分頁（${JSON.stringify(st.tabs)}）`);
    check(st.panes > 0 && st.floor && st.leftTabs && st.outputVisible, `頁面其餘部分正常：pane 列、Factory Floor、左欄分頁、Live Output 面板（${JSON.stringify({ ...st, tabs: undefined })}）`);
    const after = ctx.console.filter((e) => e.at >= tReload);
    check(after.some((e) => e.level === 'warning'), `console 有警告（載入後的 console：${JSON.stringify(after.slice(0, 5))}）`);
    check(!after.some((e) => e.level === 'exception'), '載入後沒有未捕捉的例外');
  });
}

// git-review task 4.1：GIVEN 瀏覽器本機儲存中是本 change 之前的格式（v1，記錄了 README.md 與 docs/a.md
// 兩個檔案分頁，沒有 `kind` 欄位）WHEN 載入頁面 THEN 兩個檔案分頁依原順序還原並顯示內容。在導覽前用
// `Page.addScriptToEvaluateOnNewDocument` 直接寫入 v1 格式的 localStorage 值（早於 files.js 的
// restoreTabs()），模擬「使用者在 git-review 之前就打開過這兩個分頁」；不透過 UI 操作開啟，才是真的在測
// 「讀到舊格式」而不是「這次執行期間自己寫入又讀回」。
async function segRestoreOldFormat() {
  await withCockpit(
    'restore-v1',
    {
      beforeNavigate: async (ctx) => {
        const root = await rootInfo(ctx, PANE_REVIEW);
        const v1 = {
          v: 1,
          tabs: [
            { runtime: RUNTIME, rootId: root.root_id, path: 'README.md', rootName: root.name },
            { runtime: RUNTIME, rootId: root.root_id, path: 'docs/a.md', rootName: root.name },
          ],
          current: { runtime: RUNTIME, rootId: root.root_id, path: 'docs/a.md' },
          left: 'files',
        };
        await ctx.cdp.send('Page.addScriptToEvaluateOnNewDocument', {
          source: `try { localStorage.setItem('cockpit.fileTabs', ${JSON.stringify(JSON.stringify(v1))}); } catch (e) {}`,
        });
        log(`已在導覽前寫入 v1 格式的 localStorage（root_id=${root.root_id}）`);
      },
    },
    async (ctx) => {
      const restored = await ctx.cdp.poll(() => window.__fc.tabsInfo().filter((t) => !t.live).length >= 2, [], UI_TIMEOUT_MS);
      const tabs = await tabsInfo(ctx);
      check(
        !!restored && JSON.stringify(tabs.map((t) => (t.live ? 'LIVE' : t.path))) === JSON.stringify(['LIVE', 'README.md', 'docs/a.md']),
        `舊格式（v1，無 kind 欄位）的兩個檔案分頁依原順序還原（實際 ${JSON.stringify(tabs)}）`
      );
      const cur = tabs.find((t) => t.selected);
      check(!!cur && cur.path === 'docs/a.md', `目前分頁為 docs/a.md（實際 ${JSON.stringify(cur)}）`);
      check(!!(await waitPanelText(ctx, '一個很短的 Markdown 檔案')), '目前分頁（docs/a.md）顯示內容');
      await clickTab(ctx, 'README.md');
      check(!!(await waitPanelText(ctx, 'review-repo')), 'README.md 分頁切過去後也顯示內容');
    }
  );
}

// ---------------------------------------------------------------------------
// file-review：在 VS Code 開啟
// ---------------------------------------------------------------------------

// GIVEN runtime win 的根目錄、分頁為 docs/a b.md WHEN 查詢其中繼資料並檢查連結 THEN vscode_uri 與連結 href 皆為
// vscode://file/<磁碟>:/…/docs/a%20b.md（spec 例中的 D:/repo 在這裡是暫存副本的主機路徑）。
async function segVscodeWindows() {
  await withCockpit('vscode-win', { beforeLoad: (preview) => writeTemp(preview, 'docs/a b.md', '# a b\n\n檔名有空白（files-check）。\n') }, async (ctx) => {
    const root = await rootInfo(ctx, PANE_REVIEW);
    const meta = await metaOf(ctx, root.root_id, 'docs/a b.md');
    const segs = path.resolve(ctx.preview.reviewRepo).split(/[\\/]+/);
    const expected = `vscode://file/${segs[0]}/${segs.slice(1).map(encodeURIComponent).join('/')}/docs/a%20b.md`;
    check(meta.status === 200 && meta.body.vscode_uri === expected, `中繼資料 vscode_uri＝${expected}（實際 ${meta.body && meta.body.vscode_uri}）`);
    check(meta.body && /^vscode:\/\/file\/[A-Za-z]:\//.test(meta.body.vscode_uri) && meta.body.vscode_uri.endsWith('/review-repo/docs/a%20b.md'), 'vscode_uri 形狀為 vscode://file/<磁碟>:/…/review-repo/docs/a%20b.md');
    await openTree(ctx);
    await openFile(ctx, 'docs/a b.md');
    const href = await ctx.cdp.poll(() => (window.__fc.vscodeLink() ? window.__fc.vscodeLink().getAttribute('href') : false), [], UI_TIMEOUT_MS);
    need(!!href, `工具列有「在 VS Code 開啟」連結（契約 C5）；目前分頁文字「${(await panelText(ctx)).slice(0, 120)}」`);
    check(href === meta.body.vscode_uri, `連結 href 等於中繼資料的 vscode_uri（href ${href}）`);
  });
}

// GIVEN runtime wsl（distro Ubuntu-24.04）、分頁為 a.md WHEN 查詢其中繼資料並檢查連結 THEN vscode_uri 與連結 href 皆為
// vscode://vscode-remote/wsl+Ubuntu-24.04/home/u/repo/a.md:1。ui_preview 沒有 WSL runtime：以 CDP Fetch 把 a.md 的
// 中繼資料回應改成 spec 的 WSL 形狀，斷言前端照原值顯示在 href。
async function segVscodeWsl() {
  await withCockpit(
    'vscode-wsl',
    {
      beforeLoad: (preview) => writeTemp(preview, 'a.md', '# a.md\n\nWSL 形狀的 vscode_uri 由腳本攔截改寫（files-check）。\n'),
      beforeNavigate: async (ctx) => {
        ctx.rewrite = await installMetaRewrite(ctx.cdp, 'a.md', WSL_VSCODE_URI);
      },
    },
    async (ctx) => {
      await openTree(ctx);
      await openFile(ctx, 'a.md');
      const href = await ctx.cdp.poll(() => (window.__fc.vscodeLink() ? window.__fc.vscodeLink().getAttribute('href') : false), [], UI_TIMEOUT_MS);
      need(!!href, `工具列有「在 VS Code 開啟」連結（契約 C5）；目前分頁文字「${(await panelText(ctx)).slice(0, 120)}」`);
      check(ctx.rewrite.count() >= 1, `頁面讀到的 a.md 中繼資料已被改寫成 WSL 形狀（攔截 ${ctx.rewrite.count()} 次）`);
      check(href === WSL_VSCODE_URI, `連結 href＝${WSL_VSCODE_URI}（實際 ${href}）`);
    }
  );
}

// ---------------------------------------------------------------------------
// live-output delta
// ---------------------------------------------------------------------------

// GIVEN 已選定 w1:p1（wJ:p4），目前分頁是某個檔案分頁 WHEN 點 w1:p2（wJ:p1）列 THEN 目前分頁改為 Live Output，
// 面板顯示 wJ:p1，檔案分頁仍在分頁列上。
async function segSelectSwitchesToLive() {
  await withCockpit('lo-switch', {}, async (ctx) => {
    await openTree(ctx);
    await openFile(ctx, 'README.md');
    const pre = await ctx.cdp.run(() => ({ live: window.__fc.liveTab() ? window.__fc.liveTab().getAttribute('aria-selected') : null, out: window.__fc.visible(document.getElementById('output')) }));
    need(pre.live !== null && pre.live !== 'true' && !pre.out, `前置：目前分頁是檔案分頁、Live Output 面板不可見（${JSON.stringify(pre)}）`);
    await selectPane(ctx, PANE_TICKER);
    const live = await ctx.cdp.poll(() => !!window.__fc.liveTab() && window.__fc.liveTab().getAttribute('aria-selected') === 'true', [], UI_TIMEOUT_MS);
    check(!!live, '目前分頁改為 Live Output');
    const st = await ctx.cdp.run(() => ({
      outputVisible: window.__fc.visible(document.getElementById('output')),
      title: document.querySelector('#output .output-title') ? document.querySelector('#output .output-title').textContent : null,
      readme: !!window.__fc.fileTab('README.md'),
    }));
    check(st.outputVisible && typeof st.title === 'string' && st.title.includes(PANE_TICKER), `Live Output 面板可見並顯示 ${PANE_TICKER}（${JSON.stringify(st)}）`);
    check(st.readme, 'README.md 檔案分頁仍在分頁列上');
  });
}

// 輸出請求跨分頁切換的分析（純函式；fix round 1 起，fix round 2 依控制端裁決 R23 改寫）。recs 為頁面端請求記錄
// （pageHelpers 的 window.__fcRequests，已篩成輸出請求），每筆的 `at.live`／`endAt.live` 是發出當下／settle 當下
// Live Output tab 的 aria-selected（'true'／'false'），在請求發出的同一個同步區塊內讀取，不依賴任何非同步回呼的時間。
//   - startedWhileAway：發出當下 Live Output 不是目前分頁（at.live === 'false'）——spec 要求 0。
//   - completedWhileAway：發出當下 Live Output 是目前分頁、settle 當下已不是（切走前已發出、切走後才完成，成功或被
//     中止都算）——spec 允許至多 1。
//   - immediateMs：最後一次點 Live Output tab（capture 階段 click 的時間，在前端 handler 之前同步記錄）之後，第一個
//     在 Live Output 可見時發出的請求相距的毫秒數（沒有則 null）。
const IMMEDIATE_MS = 250; // 「切回後立即」：點擊處理當下就排請求的實作在數 ms 內送出；等原本 1 秒輪詢計時一定超過。
function analyzeOutputAcrossSwitch(recs, tBackClick) {
  const startedWhileAway = recs.filter((r) => r.at && r.at.live === 'false');
  const completedWhileAway = recs.filter((r) => r.at && r.at.live === 'true' && r.end !== null && r.endAt && r.endAt.live === 'false');
  const back = tBackClick === null ? [] : recs.filter((r) => r.at && r.at.live === 'true' && r.start >= tBackClick).sort((a, b) => a.start - b.start);
  return {
    startedWhileAway: startedWhileAway.map((r) => `${r.id}:${r.via || 'fetch'}`),
    completedWhileAway: completedWhileAway.map((r) => r.id),
    immediateMs: back[0] ? Math.round((back[0].start - tBackClick) * 10) / 10 : null,
  };
}
async function pageRequests(ctx) {
  return ctx.cdp.run(() => ({ recs: window.__fcRequests.slice(), liveClicks: window.__fcLiveClicks.slice() }));
}
// CDP 總數健全檢查（控制端裁決 R23）：CDP 看到的某類請求總數必須等於頁面端記錄的總數；不等代表有請求走了
// 未包到的途徑（例如 <img>、sendBeacon、worker），頁面端的分類對它無效，判 FAIL。在應該安靜的時段呼叫
// （沒有該類輪詢時）：先讀頁面端 P1，等 400 ms 讓 CDP 事件到齊，讀 CDP 總數 C，再讀頁面端 P2；要求 P1＝P2＝C。
// CDP 端以 sessionId＋requestId 去重（redirect 會對同一個 requestId 再送一次 requestWillBeSent）。
async function cdpPageCountSanity(ctx, kind) {
  const count = async () => (await pageRequests(ctx)).recs.filter((r) => classify(r.url) === kind).length;
  const p1 = await count();
  await sleep(400);
  const cdp = new Set(ctx.net.reqs.filter((r) => r.kind === kind).map((r) => r.key)).size;
  const p2 = await count();
  return { kind, page: p1, pageAfter: p2, cdp, ok: p1 === p2 && cdp === p1 };
}

// GIVEN 已選定一個 pane，之後切到某個檔案分頁 WHEN 觀察 10 秒後切回 Live Output THEN 切走後至多再完成一個先前已
// 發出的請求，之後直到切回前沒有輸出請求；切回後立即收到一個輸出請求。
// fix round 2（控制端裁決 R23）：每個輸出請求（fetch 與 XHR）在發出當下同步記下 Live Output 是否為目前分頁，
// 分類只看這個，不再用 MutationObserver 回呼時間當邊界（見 pageHelpers 與 analyzeOutputAcrossSwitch）。
// 延遲回應情境：`wJ:p4=delay:1500`，並等到確實有一個剛發出、尚未回應的輸出請求才切走，讓「切走後完成一個既有
// 請求」這條路徑真的被走到。CDP 只做總數健全檢查（在檔案分頁期間、應該沒有輸出輪詢時比對）。
async function segNoOutputWhileFileTab() {
  await withCockpit('lo-noreq', { env: { COCKPIT_PREVIEW_OUTPUT_MODES: `${PANE_REVIEW}=delay:1500` } }, async (ctx) => {
    await openTree(ctx);
    const tSel = Date.now();
    await sleep(2500);
    need(ctx.net.since(tSel, 'output').length >= 1, `前置：選定 ${PANE_REVIEW} 後頁面在請求輸出（2.5 秒內 ${ctx.net.since(tSel, 'output').length} 次）`);
    await waitRow(ctx, 'README.md');
    const inflight = await ctx.cdp.poll(
      () => window.__fcRequests.some((r) => /\/panes\/[^/]+\/output$/.test(new URL(r.url).pathname) && r.end === null && performance.now() - r.start < 700),
      [],
      6000,
      20
    );
    need(!!inflight, '前置（延遲回應情境）：切走前確實有一個剛發出、尚未回應的輸出請求');
    await openFile(ctx, 'README.md');
    await sleep(10000);
    const sanity = await cdpPageCountSanity(ctx, 'output');
    await clickTab(ctx, 'LIVE');
    await sleep(1500);
    const p = await pageRequests(ctx);
    const recs = p.recs.filter((r) => classify(r.url) === 'output');
    const tBackClick = p.liveClicks.length ? p.liveClicks[p.liveClicks.length - 1] : null;
    need(tBackClick !== null, '記錄到點 Live Output tab 的時間');
    const a = analyzeOutputAcrossSwitch(recs, tBackClick);
    log(`輸出請求跨切換分析：${JSON.stringify(a)}；CDP 健全檢查：${JSON.stringify(sanity)}`);
    need(a.completedWhileAway.length >= 1, `延遲回應情境成立：有一個切走前發出的請求在切走後才完成（${JSON.stringify(a.completedWhileAway)}）`);
    check(a.completedWhileAway.length <= 1, `切走後至多再完成一個切走前已發出的請求（實際 ${JSON.stringify(a.completedWhileAway)}）`);
    check(a.startedWhileAway.length === 0, `檔案分頁期間沒有發出任何輸出請求（fetch／XHR；實際 ${JSON.stringify(a.startedWhileAway)}）`);
    check(sanity.ok, `CDP 總數健全檢查：CDP 看到的輸出請求數等於頁面端記錄數（${JSON.stringify(sanity)}）`);
    check(a.immediateMs !== null && a.immediateMs <= IMMEDIATE_MS, `切回後立即發出一個輸出請求（${a.immediateMs === null ? '1.5 秒內沒有' : `點擊後 ${a.immediateMs} ms`}，門檻 ${IMMEDIATE_MS} ms）`);
  });
}

// GIVEN 已選定一個每秒多印一行的 pane，面板停在最底端 WHEN 切到某個檔案分頁 5 秒後切回 THEN 切回後面板在最底端，並在
// 3 秒內出現切走期間印出的行。用 long 模式（300+ 行，確保內容超過一屏，「在最底端」不是恆真）。
async function segStickToBottomAcrossTabs() {
  await withCockpit('lo-bottom', { env: { COCKPIT_PREVIEW_OUTPUT_MODES: `${PANE_REVIEW}=long` } }, async (ctx) => {
    await openTree(ctx);
    const ready = await ctx.cdp.poll(() => {
      const pre = document.querySelector('#output .output-text');
      return !!pre && pre.scrollHeight > pre.clientHeight + 50;
    }, [], UI_TIMEOUT_MS);
    need(!!ready, '前置：面板內容超過一屏');
    await ctx.cdp.run(() => {
      const pre = document.querySelector('#output .output-text');
      pre.scrollTop = pre.scrollHeight;
      return true;
    });
    await sleep(1200);
    const readBottom = () =>
      ctx.cdp.run(() => {
        const pre = document.querySelector('#output .output-text');
        const m = pre.textContent.match(/line (\d+)\s*$/);
        return { gap: pre.scrollHeight - pre.scrollTop - pre.clientHeight, last: m ? Number(m[1]) : null, visible: window.__fc.visible(pre) };
      });
    const b0 = await readBottom();
    need(b0.gap <= NEAR_BOTTOM_PX && b0.last !== null, `前置：面板停在最底端（${JSON.stringify(b0)}）`);
    await openFile(ctx, 'README.md');
    await sleep(5000);
    await clickTab(ctx, 'LIVE');
    const b1 = await readBottom();
    check(b1.visible && b1.gap <= NEAR_BOTTOM_PX, `切回後面板在最底端（距底 ${b1.gap} px，門檻 ${NEAR_BOTTOM_PX}）`);
    const expectLine = b0.last + 4;
    const shown = await ctx.cdp.poll((n) => {
      const pre = document.querySelector('#output .output-text');
      const m = pre.textContent.match(/line (\d+)\s*$/);
      return !!m && Number(m[1]) >= n && pre.scrollHeight - pre.scrollTop - pre.clientHeight <= 6;
    }, [expectLine], 3000);
    const b2 = await readBottom();
    check(!!shown, `3 秒內出現切走期間印出的行（最後一行 ≥ line ${expectLine}）且仍在最底端（${JSON.stringify(b2)}）`);
  });
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

const SEGMENTS = [
  { code: 'self/鷹架', fn: segSelfScaffold, self: true },
  { code: 'self/段落代號', fn: segSelfSegmentArg, self: true },
  { code: 'file-review/左欄三個分頁', fn: segLeftThreeTabs },
  { code: 'file-review/切到檔案分頁', fn: segSwitchToFilesTab },
  { code: 'file-review/沒有選定 pane', fn: segNoPaneSelected },
  { code: 'file-review/展開狀態跨根目錄保留', fn: segExpandStatePerRoot },
  { code: 'file-review/重畫不影響檔案樹', fn: segRepaintKeepsTree },
  { code: 'file-review/同一 pane 改 cwd 後檔案樹跟著換根', fn: segCwdChangeRehomesTree },
  { code: 'file-review/開檔新增分頁', fn: segOpenAddsTab },
  { code: 'file-review/重複開啟不新增', fn: segReopenNoNewTab },
  { code: 'file-review/關閉目前分頁', fn: segCloseCurrentTab },
  { code: 'file-review/切換 Project 不影響分頁', fn: segProjectSwitchKeepsTabs },
  { code: 'file-review/md 相對連結在分頁區開啟', fn: segMdRelativeLink },
  { code: 'file-review/外部圖片不載入', fn: segExternalImageBlocked },
  { code: 'file-review/HTML 內的腳本不執行', fn: segHtmlScriptBlocked },
  { code: 'file-review/沒有宣告編碼的 UTF-8 HTML', fn: segUtf8HtmlWithoutMeta },
  { code: 'file-review/中文 PDF', fn: segChinesePdf },
  { code: 'file-review/純文字不被解讀', fn: segPlainTextNotParsed },
  { code: 'file-review/改檔後更新並保住捲動', fn: segAutoUpdateKeepsScroll },
  { code: 'file-review/Live Output 分頁時不查詢', fn: segNoMetaOnLiveTab },
  { code: 'file-review/檔案被刪掉後又出現', fn: segDeletedThenRecreated },
  { code: 'file-review/重新整理後還原', fn: segRestoreAfterReload },
  { code: 'file-review/儲存內容損毀', fn: segCorruptStorage },
  { code: 'file-review/舊格式照常還原', fn: segRestoreOldFormat },
  { code: 'file-review/Windows 檔案', fn: segVscodeWindows },
  { code: 'file-review/WSL 檔案', fn: segVscodeWsl },
  { code: 'live-output/選定 pane 時切回 Live Output 分頁', fn: segSelectSwitchesToLive },
  { code: 'live-output/檔案分頁期間不請求輸出', fn: segNoOutputWhileFileTab },
  { code: 'live-output/切回時保持貼底', fn: segStickToBottomAcrossTabs },
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
  // 環境檢查：不動別人的行程，有衝突就停。
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
  log(`截圖目錄：${SCRATCH}`);
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
  const selfRows = summary.filter((s) => s.self);
  const scenRows = summary.filter((s) => !s.self);
  console.log(`自我測試段：${selfRows.filter((s) => s.fails === 0).length}/${selfRows.length} PASS；scenario 段：${scenRows.filter((s) => s.fails > 0).length}/${scenRows.length} FAIL（前端尚未實作的 scenario 應 FAIL）；收尾衛生：${hygieneFails === 0 ? 'PASS' : 'FAIL'}`);
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
