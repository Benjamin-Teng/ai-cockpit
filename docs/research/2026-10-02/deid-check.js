#!/usr/bin/env node
// 去識別化檢查：推送前確認 repo 內沒有個人識別字串（見 deid-check.md）。
//
// 所有敏感字串都在執行時取得（作業系統、WSL、git 設定、本機詞表 .deid-terms），
// 本檔不含任何實際值；輸出只印類別代號（U、W、H、E、T1..Tn），絕不印出詞本身。
//
// 用法：
//   node docs/research/2026-10-02/deid-check.js [--rev <commit>] [--history] [--distro <name>]
// exit code：0 無命中；1 有命中（或歷史模式下有 refs/heads|remotes|tags/ 以外的 ref）；2 用法或執行錯誤。

'use strict';

const os = require('node:os');
const fs = require('node:fs');
const path = require('node:path');
const { spawn, execFileSync } = require('node:child_process');

const DEFAULT_DISTRO = 'Ubuntu-24.04';
const USAGE = [
  '用法：node docs/research/2026-10-02/deid-check.js [選項]',
  '  --rev <commit>    檢查指定 commit 的所有檔案（預設 HEAD；與 --history 互斥）',
  '  --history         掃描全部 git 物件（blob、tree 檔名、commit、tag，含不可達）並檢查 ref',
  '  --distro <name>   取得 W 用的 WSL distro（預設 ' + DEFAULT_DISTRO + '，亦可用環境變數 DEID_WSL_DISTRO）',
  '  -h, --help        顯示本說明',
].join('\n');

class UsageError extends Error {}

function parseArgs(argv) {
  const opts = { rev: null, history: false, distro: process.env.DEID_WSL_DISTRO || DEFAULT_DISTRO, help: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const valueOf = (name) => {
      if (a.startsWith(name + '=')) return a.slice(name.length + 1);
      const v = argv[++i];
      if (v === undefined || v.startsWith('-')) throw new UsageError(name + ' 需要一個值');
      return v;
    };
    if (a === '-h' || a === '--help') opts.help = true;
    else if (a === '--history') opts.history = true;
    else if (a === '--rev' || a.startsWith('--rev=')) opts.rev = valueOf('--rev');
    else if (a === '--distro' || a.startsWith('--distro=')) opts.distro = valueOf('--distro');
    else throw new UsageError('未知的參數：' + a);
  }
  if (opts.history && opts.rev !== null) throw new UsageError('--history 與 --rev 不可同時使用');
  return opts;
}

// ---------------------------------------------------------------- git 與詞表

let repoRoot = null;

function git(args) {
  return execFileSync('git', args, {
    cwd: repoRoot || undefined,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    maxBuffer: 1 << 29,
    windowsHide: true,
  });
}

function gitOrNull(args) {
  try {
    return git(args).trim();
  } catch {
    return null;
  }
}

// noreply、anthropic.com、example／.invalid 網域的 email 不是個人識別字串
function isIgnorableEmail(email) {
  const e = email.toLowerCase();
  const domain = e.slice(e.lastIndexOf('@') + 1);
  return (
    e.includes('noreply') ||
    domain === 'anthropic.com' ||
    domain.endsWith('.anthropic.com') ||
    /(^|\.)example(\.[a-z]+)?$/.test(domain) ||
    domain.endsWith('.invalid') ||
    domain === 'invalid'
  );
}

// .deid-terms 的編碼：UTF-16LE（FF FE）、UTF-16BE（FE FF）、UTF-8（有無 BOM 皆可）；
// 沒有 BOM 但含 NUL 位元組視為 UTF-16LE（Windows PowerShell 5.1 的 > 與 Out-File 預設輸出）。
function decodeTermsFile(buf) {
  if (buf.length >= 2 && buf[0] === 0xff && buf[1] === 0xfe) {
    return buf.subarray(2).toString('utf16le');
  }
  if (buf.length >= 2 && buf[0] === 0xfe && buf[1] === 0xff) {
    const body = Buffer.from(buf.subarray(2, 2 + ((buf.length - 2) & ~1))); // 複製後再原地交換位元組
    return body.swap16().toString('utf16le');
  }
  if (buf.length >= 3 && buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf) {
    return buf.subarray(3).toString('utf8');
  }
  if (buf.includes(0)) return buf.toString('utf16le');
  return buf.toString('utf8');
}

// 回傳 { terms: [{code, value, boundary}], warnings: [] }；value 只在記憶體內使用
function gatherTerms(distro) {
  const terms = [];
  const warnings = [];
  const add = (code, value, boundary) => {
    if (!value) return;
    if (terms.some((t) => t.code === code && t.value.toLowerCase() === value.toLowerCase())) return;
    terms.push({ code, value, boundary: !!boundary });
  };

  try {
    add('U', os.userInfo().username);
  } catch {
    warnings.push('U 無法取得，已略過');
  }
  // H：完整主機名稱，加上 NetBIOS 名稱（COMPUTERNAME，最長 15 字，常是完整名稱被截斷）。
  // 兩者不分大小寫去重；若其中一個包含另一個，只留較短者（較短者的子字串比對已涵蓋較長者）。
  const hostNames = [os.hostname(), process.env.COMPUTERNAME]
    .filter(Boolean)
    .filter((h, i, arr) => arr.findIndex((o) => o.toLowerCase() === h.toLowerCase()) === i);
  for (const h of hostNames) {
    if (!hostNames.some((o) => o !== h && h.toLowerCase().includes(o.toLowerCase()))) add('H', h);
  }

  try {
    const out = execFileSync('wsl.exe', ['-d', distro, '--exec', 'whoami'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      timeout: 30000,
      windowsHide: true,
    });
    const w = out.replace(/\0/g, '').trim();
    if (w) add('W', w, true);
    else warnings.push('W 取得結果為空，已略過');
  } catch {
    warnings.push('W 已略過：WSL 不可用或 distro「' + distro + '」不存在（可用 --distro 指定）');
  }

  for (const args of [['config', '--global', 'user.email'], ['config', 'user.email']]) {
    const email = gitOrNull(args);
    if (email && !isIgnorableEmail(email)) add('E', email);
  }

  const termsFile = path.join(repoRoot, '.deid-terms');
  if (fs.existsSync(termsFile)) {
    const text = decodeTermsFile(fs.readFileSync(termsFile));
    let n = 0;
    for (const raw of text.split(/\r?\n/)) {
      const line = raw.trim();
      if (!line || line.startsWith('#')) continue;
      // 解碼失敗的殘渣（NUL、U+FFFD）若當成詞，會變成亂碼詞而真詞漏抓；寧可中止
      if (/[\0�]/.test(line)) throw new Error('.deid-terms 含無法解碼的字元（請存成 UTF-8 或 UTF-16）');
      n++;
      terms.push({ code: 'T' + n, value: line, boundary: false });
    }
  } else {
    warnings.push('找不到 .deid-terms（repo 根目錄的本機詞表），T 類別未檢查');
  }
  return { terms, warnings };
}

// ---------------------------------------------------------------- 比對

// 位元組以 latin1 字串（一位元組一字元）比對；每個字元用 \xHH 展開，不依賴 regex 跳脫。
// 大小寫：每個字元取 原樣／小寫／大寫 的編碼做 alternation。
function encodeChar(ch, enc) {
  return Buffer.from(ch, enc)
    .toString('hex')
    .replace(/../g, (h) => '\\x' + h);
}

function buildPattern(term, enc) {
  let body = '';
  for (const cp of Array.from(term.value)) {
    const variants = new Set([cp, cp.toLowerCase(), cp.toUpperCase()]);
    const alts = [...variants].filter((v) => Array.from(v).length === 1).map((v) => encodeChar(v, enc));
    body += alts.length === 1 ? alts[0] : '(?:' + alts.join('|') + ')';
  }
  if (!term.boundary) return body;
  const word = '[A-Za-z0-9_]';
  return enc === 'utf16le'
    ? '(?<!' + word + '\\x00)' + body + '(?!' + word + '\\x00)'
    : '(?<!' + word + ')' + body + '(?!' + word + ')';
}

function compileTerms(terms) {
  return terms.map((t) => ({
    code: t.code,
    regexes: ['utf8', 'utf16le'].map((enc) => new RegExp(buildPattern(t, enc), 'g')),
  }));
}

// 回傳 Map<code, count>
function scanBuffer(compiled, buf) {
  const hits = new Map();
  if (buf.length === 0) return hits;
  const s = buf.toString('latin1');
  for (const c of compiled) {
    let n = 0;
    for (const re of c.regexes) n += (s.match(re) || []).length;
    if (n > 0) hits.set(c.code, (hits.get(c.code) || 0) + n);
  }
  return hits;
}

// ---------------------------------------------------------------- git cat-file --batch 串流

// 以單一長壽 git 行程讀物件；onObject({oid, type, size}, bodyBuffer) 同步處理。
function runBatch(args, inputText, onObject, onMissing) {
  return new Promise((resolve, reject) => {
    const child = spawn('git', args, {
      cwd: repoRoot,
      stdio: [inputText === null ? 'ignore' : 'pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });
    let stderr = '';
    let failed = false;
    const fail = (err) => {
      if (failed) return;
      failed = true;
      child.kill();
      reject(err);
    };
    child.on('error', fail);
    child.stderr.on('data', (d) => {
      stderr += d;
    });
    if (inputText !== null) {
      child.stdin.on('error', () => {});
      child.stdin.end(inputText);
    }

    const EMPTY = Buffer.alloc(0);
    let head = EMPTY;
    let state = 'head';
    let cur = null;
    let need = 0;
    let have = 0;
    let chunks = [];

    child.stdout.on('data', (chunk) => {
      if (failed) return;
      try {
        let data = chunk;
        while (data.length > 0) {
          if (state === 'head') {
            const buf = head.length > 0 ? Buffer.concat([head, data]) : data;
            const nl = buf.indexOf(10);
            if (nl < 0) {
              head = buf;
              return;
            }
            head = EMPTY;
            const line = buf.toString('latin1', 0, nl);
            data = buf.subarray(nl + 1);
            const m = /^([0-9a-f]+) (\w+) (\d+)$/.exec(line);
            if (m) {
              cur = { oid: m[1], type: m[2], size: Number(m[3]) };
              need = cur.size + 1; // 物件後固定多一個 LF
              have = 0;
              chunks = [];
              state = 'body';
            } else if (/^[0-9a-f]+ missing$/.test(line)) {
              onMissing(line.split(' ')[0]);
            } else {
              throw new Error('無法解析 git cat-file 輸出的標頭');
            }
          } else {
            const take = Math.min(need - have, data.length);
            chunks.push(data.subarray(0, take));
            have += take;
            data = data.subarray(take);
            if (have === need) {
              const body = (chunks.length === 1 ? chunks[0] : Buffer.concat(chunks)).subarray(0, cur.size);
              chunks = [];
              state = 'head';
              onObject(cur, body);
            }
          }
        }
      } catch (err) {
        fail(err);
      }
    });

    child.on('close', (code) => {
      if (failed) return;
      if (code !== 0) return fail(new Error('git ' + args[0] + ' 失敗（exit ' + code + '）：' + stderr.trim()));
      if (state !== 'head' || head.length > 0) return fail(new Error('git cat-file 輸出被截斷'));
      resolve();
    });
  });
}

// tree 物件的內容含 20 位元組（sha256 為 32）二進位雜湊；只取檔名比對，避免雜湊誤中
function treeNames(buf, hashLen) {
  const names = [];
  let i = 0;
  while (i < buf.length) {
    const sp = buf.indexOf(0x20, i);
    const nul = sp < 0 ? -1 : buf.indexOf(0, sp);
    if (nul < 0) break;
    names.push(buf.subarray(sp + 1, nul), Buffer.from('\n'));
    i = nul + 1 + hashLen;
  }
  return Buffer.concat(names);
}

// ---------------------------------------------------------------- 輸出

class Report {
  constructor(compiled) {
    this.compiled = compiled;
    this.lines = [];
    // code -> { items: Set(key), total: number, byType: Map(type -> Set(key)) }
    this.cats = new Map();
  }

  // 路徑本身若含詞，輸出時隱藏（絕不印出詞）
  safe(text) {
    return scanBuffer(this.compiled, Buffer.from(text, 'utf8')).size > 0 ? '<名稱含詞，已隱藏>' : text;
  }

  record(key, type, hits) {
    for (const [code, n] of hits) {
      if (!this.cats.has(code)) this.cats.set(code, { items: new Set(), total: 0, byType: new Map() });
      const c = this.cats.get(code);
      c.items.add(key);
      c.total += n;
      if (!c.byType.has(type)) c.byType.set(type, new Set());
      c.byType.get(type).add(key);
    }
  }

  fmt(hits) {
    return [...hits].map(([code, n]) => code + ':' + n).join(' ');
  }
}

function sortCodes(codes) {
  const rank = (c) => (c === 'U' ? 0 : c === 'W' ? 1 : c === 'H' ? 2 : c === 'E' ? 3 : 4 + Number(c.slice(1)));
  return [...codes].sort((a, b) => rank(a) - rank(b));
}

function printSummary(out, report, unitName, scanned, expectedCodes) {
  out('');
  out('掃描單位：' + scanned + ' 個' + unitName);
  out('已載入類別：' + sortCodes(expectedCodes).join(' '));
  const hitCodes = sortCodes(report.cats.keys());
  if (hitCodes.length === 0) {
    out('命中：無');
    return;
  }
  out('命中摘要（類別：不同' + unitName + '數／出現次數）：');
  for (const code of hitCodes) {
    const c = report.cats.get(code);
    const types = [...c.byType].map(([t, s]) => t + ' ' + s.size).join('、');
    out('  ' + code + '：' + c.items.size + ' 個／' + c.total + ' 次（' + types + '）');
  }
}

// ---------------------------------------------------------------- 兩種模式

async function runFileMode(rev, compiled, expectedCodes, out) {
  if (rev.startsWith('-')) throw new UsageError('--rev 的值不可以 - 開頭');
  let commit;
  try {
    commit = git(['rev-parse', '--verify', rev + '^{commit}']).trim();
  } catch {
    throw new UsageError('找不到 commit：' + rev);
  }
  out('模式：檔案（commit ' + commit.slice(0, 12) + '）');

  const listing = git(['ls-tree', '-r', '-z', commit]);
  const byOid = new Map(); // oid -> [path]
  let files = 0;
  for (const rec of listing.split('\0')) {
    if (!rec) continue;
    const tab = rec.indexOf('\t');
    const [, type, oid] = rec.slice(0, tab).split(' ');
    if (type !== 'blob') continue; // 略過 submodule（commit）
    files++;
    const p = rec.slice(tab + 1);
    if (!byOid.has(oid)) byOid.set(oid, []);
    byOid.get(oid).push(p);
  }

  const report = new Report(compiled);
  const found = [];
  const missing = [];
  // tree 為空的 commit 沒有任何 blob；空輸入送給 cat-file 會得到無法解析的回應，直接略過
  if (byOid.size > 0) {
    await runBatch(
      ['cat-file', '--batch'],
      [...byOid.keys()].join('\n') + '\n',
      (meta, body) => {
        const hits = scanBuffer(compiled, body);
        for (const p of byOid.get(meta.oid)) {
          const pathHits = scanBuffer(compiled, Buffer.from(p, 'utf8')); // 檔名本身也算
          const all = new Map(hits);
          for (const [c, n] of pathHits) all.set(c, (all.get(c) || 0) + n);
          if (all.size > 0) {
            found.push({ p, all });
            report.record(p, 'file', all);
          }
        }
      },
      (oid) => missing.push(oid),
    );
  }
  if (missing.length > 0) throw new Error('有 ' + missing.length + ' 個物件讀不到（shallow 或 promisor repo？）');

  found.sort((a, b) => (a.p < b.p ? -1 : 1));
  for (const f of found) out(report.safe(f.p) + '\t' + report.fmt(f.all));
  printSummary(out, report, '檔案', files, expectedCodes);
  return report.cats.size > 0;
}

async function runHistoryMode(compiled, expectedCodes, out) {
  out('模式：歷史（全部物件）');
  let bad = false;

  // ref 檢查：refs/heads/、refs/remotes/、refs/tags/ 是正常的（推送後必有 remote 與 tag）；
  // 其他命名空間（refs/codex/、refs/stash、refs/original/ 等）可能讓舊物件保持可達，算命中。
  // 全部物件本來就會被掃描，這裡只擋「不該存在或不該推送的 ref」。
  const report = new Report(compiled);
  const refs = git(['for-each-ref', '--format=%(refname)']).split('\n').filter(Boolean);
  const normalRef = (r) => ['refs/heads/', 'refs/remotes/', 'refs/tags/'].some((p) => r.startsWith(p));
  const odd = refs.filter((r) => !normalRef(r));
  out('ref：共 ' + refs.length + ' 個，其中 heads／remotes／tags 之外 ' + odd.length + ' 個');
  for (const r of odd) out('警告：非 heads／remotes／tags 的 ref：' + report.safe(r));
  if (odd.length > 0) bad = true;

  // 所有 ref 名稱（含 heads／remotes／tags）也要比對詞表；lightweight tag 與分支名不在任何物件內，物件掃描看不到
  for (const r of refs) {
    const hits = scanBuffer(compiled, Buffer.from(r, 'utf8'));
    if (hits.size === 0) continue;
    report.record('ref:' + r, 'ref', hits);
    out(report.safe(r) + '\tref 名稱\t' + report.fmt(hits));
  }

  // oid -> 首見路徑（只有可達物件有）；順便得到可達集合
  const reachable = new Map();
  for (const line of git(['rev-list', '--objects', '--all']).split('\n')) {
    if (!line) continue;
    const sp = line.indexOf(' ');
    const oid = sp < 0 ? line : line.slice(0, sp);
    if (!reachable.has(oid)) reachable.set(oid, sp < 0 ? null : line.slice(sp + 1));
  }

  let scanned = 0;
  const found = [];
  await runBatch(
    ['cat-file', '--batch-all-objects', '--batch'],
    null,
    (meta, body) => {
      scanned++;
      let target = body;
      if (meta.type === 'tree') target = treeNames(body, meta.oid.length / 2);
      const hits = scanBuffer(compiled, target);
      if (hits.size === 0) return;
      found.push({ meta, hits });
      report.record(meta.oid, meta.type, hits);
    },
    (oid) => {
      throw new Error('物件讀不到：' + oid);
    },
  );

  found.sort((a, b) => (a.meta.type + a.meta.oid < b.meta.type + b.meta.oid ? -1 : 1));
  for (const f of found) {
    const isReachable = reachable.has(f.meta.oid);
    const p = reachable.get(f.meta.oid);
    out(
      [
        f.meta.oid,
        f.meta.type,
        isReachable ? '可達' : '不可達',
        p ? report.safe(p) : '-',
        report.fmt(f.hits),
      ].join('\t'),
    );
  }
  printSummary(out, report, '物件', scanned, expectedCodes);
  return bad || report.cats.size > 0;
}

// ---------------------------------------------------------------- main

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (opts.help) {
    process.stdout.write(USAGE + '\n');
    return 0;
  }
  try {
    repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    }).trim();
  } catch {
    throw new UsageError('目前目錄不在 git repo 內');
  }

  const lines = [];
  const out = (s) => lines.push(s);
  const { terms, warnings } = gatherTerms(opts.distro);
  for (const w of warnings) out('警告：' + w);
  if (terms.length === 0) throw new Error('詞表為空，無從檢查');
  const compiled = compileTerms(terms);
  const expectedCodes = [...new Set(terms.map((t) => t.code))];

  const hit = opts.history
    ? await runHistoryMode(compiled, expectedCodes, out)
    : await runFileMode(opts.rev || 'HEAD', compiled, expectedCodes, out);

  out(hit ? '結果：有命中（exit 1）' : '結果：無命中（exit 0）');
  process.stdout.write(lines.join('\n') + '\n');
  return hit ? 1 : 0;
}

main().then(
  (code) => {
    process.exitCode = code;
  },
  (err) => {
    process.stderr.write('錯誤：' + (err && err.message ? err.message : String(err)) + '\n');
    if (err instanceof UsageError) process.stderr.write(USAGE + '\n');
    process.exitCode = 2;
  },
);
