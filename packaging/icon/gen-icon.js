// 從 SVG 母檔產生所有圖示產物（change app-icon design D1、D2）。改母檔後重跑；產物提交進 repo，建置不需要瀏覽器。
//
// 用法（repo 根目錄）：node packaging/icon/gen-icon.js
//   Chrome 路徑：環境變數 COCKPIT_ICON_CHROME 優先，否則依序找 Program Files、Program Files (x86)、%LOCALAPPDATA% 的 Chrome，
//   再找 Edge。以 headless 執行，不開視窗。
//
// 輸入：
//   packaging/icon/app-icon.svg        主母檔
//   packaging/icon/app-icon-small.svg  選用；存在時 24 px 以下改用它（小尺寸的簡化版）
// 輸出（覆寫）：
//   packaging/icon/app.ico              16、20、24、32、40、48、64 為 32 位元 DIB（含 AND mask），256 為 PNG 項目
//   cockpit/assets/icons/icon-192.png、icon-512.png（介面 manifest、favicon、通知）
//   site/img/icon-192.png（宣傳頁 favicon）
//
// 做法：把母檔畫進各尺寸的 <canvas>，以 toDataURL 取 PNG、getImageData 取 RGBA（未預乘 alpha，正是 ICO DIB 要的），
// 寫進 DOM 後由 --dump-dom 讀回。只用 Node 內建模組。
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFileSync } = require("child_process");

const root = path.resolve(__dirname, "..", "..");
const here = __dirname;
const DIB_SIZES = [16, 20, 24, 32, 40, 48, 64];
const PNG_ICO_SIZE = 256;
const SMALL_MAX = 24;
const WEB = [
  [192, path.join(root, "cockpit", "assets", "icons", "icon-192.png")],
  [512, path.join(root, "cockpit", "assets", "icons", "icon-512.png")],
  [192, path.join(root, "site", "img", "icon-192.png")],
];

function findChrome() {
  const env = process.env.COCKPIT_ICON_CHROME;
  if (env) return env;
  const pf = process.env.ProgramW6432 || process.env.ProgramFiles || "C:\\Program Files";
  const pf86 = process.env["ProgramFiles(x86)"] || "C:\\Program Files (x86)";
  const local = process.env.LOCALAPPDATA || "";
  const candidates = [
    path.join(pf, "Google", "Chrome", "Application", "chrome.exe"),
    path.join(pf86, "Google", "Chrome", "Application", "chrome.exe"),
    path.join(local, "Google", "Chrome", "Application", "chrome.exe"),
    path.join(pf86, "Microsoft", "Edge", "Application", "msedge.exe"),
    path.join(pf, "Microsoft", "Edge", "Application", "msedge.exe"),
  ];
  const found = candidates.find((p) => p && fs.existsSync(p));
  if (!found) throw new Error("找不到 Chrome 或 Edge；以 COCKPIT_ICON_CHROME 指定執行檔路徑");
  return found;
}

// 每個尺寸給 SVG 加上明確的 width／height，讓瀏覽器以目標解析度向量點陣化，而不是畫大圖再縮。
function sized(svg, size) {
  return svg.replace(/<svg\b/, `<svg width="${size}" height="${size}"`);
}

function renderAll() {
  const main = fs.readFileSync(path.join(here, "app-icon.svg"), "utf8");
  const smallPath = path.join(here, "app-icon-small.svg");
  const small = fs.existsSync(smallPath) ? fs.readFileSync(smallPath, "utf8") : null;
  const jobs = [];
  for (const s of DIB_SIZES) jobs.push({ size: s, rgba: true, svg: small && s <= SMALL_MAX ? small : main });
  for (const s of [PNG_ICO_SIZE, 192, 512]) jobs.push({ size: s, rgba: false, svg: main });
  const payload = jobs.map((j) => ({
    size: j.size,
    rgba: j.rgba,
    src: "data:image/svg+xml;base64," + Buffer.from(sized(j.svg, j.size)).toString("base64"),
  }));
  const html = `<!doctype html><meta charset="utf-8"><body><script>
const jobs = ${JSON.stringify(payload)};
Promise.all(jobs.map((j) => new Promise((ok, fail) => {
  const img = new Image();
  img.onload = () => {
    const c = document.createElement("canvas");
    c.width = c.height = j.size;
    const g = c.getContext("2d");
    g.drawImage(img, 0, 0, j.size, j.size);
    const out = { size: j.size, png: c.toDataURL("image/png").split(",")[1] };
    if (j.rgba) {
      const d = g.getImageData(0, 0, j.size, j.size).data;
      let s = ""; for (let i = 0; i < d.length; i++) s += String.fromCharCode(d[i]);
      out.rgba = btoa(s);
    }
    ok(out);
  };
  img.onerror = () => fail(new Error("SVG 載入失敗：" + j.size));
  img.src = j.src;
}))).then((r) => { const p = document.createElement("pre"); p.id = "out"; p.textContent = JSON.stringify(r); document.body.appendChild(p); },
         (e) => { const p = document.createElement("pre"); p.id = "err"; p.textContent = String(e); document.body.appendChild(p); });
</script>`;
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cockpit-icon-"));
  try {
    const page = path.join(tmp, "render.html");
    fs.writeFileSync(page, html);
    const dom = execFileSync(
      findChrome(),
      [
        "--headless=new",
        "--disable-gpu",
        "--no-first-run",
        "--no-default-browser-check",
        `--user-data-dir=${path.join(tmp, "profile")}`,
        "--virtual-time-budget=10000",
        "--dump-dom",
        "file:///" + page.replace(/\\/g, "/"),
      ],
      { encoding: "utf8", maxBuffer: 256 * 1024 * 1024, stdio: ["ignore", "pipe", "ignore"] },
    );
    const err = dom.match(/<pre id="err">([\s\S]*?)<\/pre>/);
    if (err) throw new Error(err[1]);
    const out = dom.match(/<pre id="out">([\s\S]*?)<\/pre>/);
    if (!out) throw new Error("Chrome 沒有輸出渲染結果（--dump-dom 內找不到 #out）");
    return JSON.parse(out[1].replace(/&quot;/g, '"').replace(/&amp;/g, "&"));
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

// 32 位元 DIB 圖示項目：BITMAPINFOHEADER（高度為兩倍，含 AND mask）＋由下而上的 BGRA＋由下而上的 1 位元 AND mask。
function dibEntry(size, rgba) {
  const header = Buffer.alloc(40);
  const maskRow = Math.ceil(size / 32) * 4;
  const pixels = Buffer.alloc(size * size * 4);
  const mask = Buffer.alloc(maskRow * size);
  for (let y = 0; y < size; y++) {
    const dstRow = size - 1 - y;
    for (let x = 0; x < size; x++) {
      const s = (y * size + x) * 4;
      const d = (dstRow * size + x) * 4;
      pixels[d] = rgba[s + 2];
      pixels[d + 1] = rgba[s + 1];
      pixels[d + 2] = rgba[s];
      pixels[d + 3] = rgba[s + 3];
      if (rgba[s + 3] === 0) mask[dstRow * maskRow + (x >> 3)] |= 0x80 >> (x & 7);
    }
  }
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(size, 4);
  header.writeInt32LE(size * 2, 8);
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);
  header.writeUInt32LE(0, 16);
  header.writeUInt32LE(pixels.length + mask.length, 20);
  return Buffer.concat([header, pixels, mask]);
}

function buildIco(entries) {
  const dir = Buffer.alloc(6 + 16 * entries.length);
  dir.writeUInt16LE(0, 0);
  dir.writeUInt16LE(1, 2);
  dir.writeUInt16LE(entries.length, 4);
  let offset = dir.length;
  entries.forEach((e, i) => {
    const o = 6 + 16 * i;
    dir.writeUInt8(e.size >= 256 ? 0 : e.size, o);
    dir.writeUInt8(e.size >= 256 ? 0 : e.size, o + 1);
    dir.writeUInt8(0, o + 2);
    dir.writeUInt8(0, o + 3);
    dir.writeUInt16LE(1, o + 4);
    dir.writeUInt16LE(32, o + 6);
    dir.writeUInt32LE(e.data.length, o + 8);
    dir.writeUInt32LE(offset, o + 12);
    offset += e.data.length;
  });
  return Buffer.concat([dir, ...entries.map((e) => e.data)]);
}

function main() {
  const results = renderAll();
  const bySize = (s, kind) => {
    const r = results.find((x) => x.size === s && (kind !== "rgba" || x.rgba));
    if (!r) throw new Error(`缺少 ${s} px 的渲染結果`);
    return Buffer.from(kind === "rgba" ? r.rgba : r.png, "base64");
  };
  const entries = DIB_SIZES.map((s) => {
    const rgba = bySize(s, "rgba");
    if (rgba.length !== s * s * 4) throw new Error(`${s} px 的 RGBA 長度不符：${rgba.length}`);
    return { size: s, data: dibEntry(s, rgba) };
  });
  entries.push({ size: PNG_ICO_SIZE, data: bySize(PNG_ICO_SIZE, "png") });
  const ico = buildIco(entries);
  fs.writeFileSync(path.join(here, "app.ico"), ico);
  console.log(`packaging/icon/app.ico  ${ico.length} bytes  [${entries.map((e) => e.size).join(", ")}]`);
  for (const [size, file] of WEB) {
    const png = bySize(size, "png");
    fs.writeFileSync(file, png);
    console.log(`${path.relative(root, file).replace(/\\/g, "/")}  ${png.length} bytes  ${size}x${size}`);
  }
}

main();
