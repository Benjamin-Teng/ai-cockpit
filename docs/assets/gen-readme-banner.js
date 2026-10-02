// 產生 README 橫幅 docs/assets/readme-banner.svg：與宣傳頁主視覺同一套點陣語彙（四條工作流經過 Plan／Build／Review）。
// 用法（repo 根目錄）：node docs/assets/gen-readme-banner.js
'use strict';
const fs = require('fs');
const path = require('path');

const C = { deep: '#091320', base: '#101a2a', text: '#e5edf3', dim: '#a3b7c9', accent: '#63d5e8', line: '#294258', ok: '#39d5ac', warn: '#e9bc73' };
const W = 1280, H = 400;
const GX = 330, GW = 890, COLS = 66, CW = GW / COLS;
const LANES = [
  { ws: 'backend', pane: 'win / wJ:p1', pos: 39, state: 'running' },
  { ws: 'docs', pane: 'win / wJ:p3', pos: 22, state: 'blocked' },
  { ws: 'qa', pane: 'wsl / wK:p2', pos: 51, state: 'running' },
  { ws: 'release', pane: 'win / wL:p1', pos: COLS - 1, state: 'completed' }
];
const TOP = 196, LANE_H = 44, ROW_H = 11;
const third = Math.floor(COLS / 3);
const stateColor = { running: C.accent, blocked: C.warn, completed: C.ok };

const out = [];
out.push(`<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-labelledby="t d">`);
out.push('<title id="t">AI Agent Cockpit</title>');
out.push('<desc id="d">Four agent workstreams shown as dot-matrix signals moving through Plan, Build and Review.</desc>');
out.push(`<style>
  .sans { font-family: "Segoe UI", -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, sans-serif; }
  .mono { font-family: "Cascadia Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
  .blink { animation: blink 1.1s ease-in-out infinite alternate; }
  @keyframes blink { from { opacity: 1; } to { opacity: 0.25; } }
  @media (prefers-reduced-motion: reduce) { .blink { animation: none; } }
</style>`);
out.push(`<rect width="${W}" height="${H}" fill="${C.deep}"/>`);
// 品牌記號：切角方框
out.push(`<path d="M61 56 h17 v17 l-5 5 h-17 v-17 z" fill="none" stroke="${C.accent}" stroke-width="1.5"/>`);
out.push(`<rect x="61" y="61" width="12" height="12" fill="${C.accent}"/>`);
out.push(`<text x="92" y="74" class="sans" font-size="20" font-weight="600" fill="${C.text}">AI Agent Cockpit</text>`);
out.push(`<text x="56" y="132" class="sans" font-size="40" font-weight="600" letter-spacing="-0.8" fill="${C.text}">See your whole agent pipeline, signal by signal.</text>`);

// 階段欄標
['Plan', 'Build', 'Review'].forEach((s, i) => {
  const x = GX + i * third * CW;
  out.push(`<line x1="${x.toFixed(1)}" y1="${TOP - 26}" x2="${x.toFixed(1)}" y2="${TOP - 12}" stroke="${C.line}"/>`);
  out.push(`<text x="${(x + 7).toFixed(1)}" y="${TOP - 15}" class="mono" font-size="14" fill="${C.dim}">${s}</text>`);
});

LANES.forEach((l, li) => {
  const y0 = TOP + li * LANE_H;
  out.push(`<text x="56" y="${y0 + 12}" class="mono" font-size="15" font-weight="600" fill="${C.text}">${l.ws}</text>`);
  out.push(`<text x="56" y="${y0 + 31}" class="mono" font-size="14" fill="${C.dim}">${l.pane} <tspan fill="${stateColor[l.state]}">${l.state}</tspan></text>`);
  const dots = [];
  for (let x = 0; x < COLS; x++) {
    for (let r = 0; r < 3; r++) {
      let col = C.line, a = 0.9, s = 2;
      const trail = l.pos - x; // 1..7：頭部後方的尾巴
      if (x === l.pos && r === 1) { col = stateColor[l.state]; s = 4; a = 1; }
      else if (x === l.pos) { col = l.state === 'blocked' ? C.warn : stateColor[l.state]; s = 3; a = 0.55; }
      else if (trail >= 1 && trail <= 7 && r === 1 && l.state !== 'completed') { col = C.accent; a = 0.85 - trail * 0.1; s = 3; }
      else if (x < l.pos && r === 1) { col = C.ok; a = 0.55; }
      if ((x === third || x === third * 2) && col === C.line) { col = C.dim; a = 0.35; }
      const cx = GX + x * CW + CW / 2, cy = y0 + 4 + r * ROW_H + ROW_H / 2;
      dots.push(`<rect x="${(cx - s / 2).toFixed(1)}" y="${(cy - s / 2).toFixed(1)}" width="${s}" height="${s}" fill="${col}"${a < 1 ? ` fill-opacity="${a.toFixed(2)}"` : ''}/>`);
    }
  }
  out.push(dots.join(''));
  if (l.state === 'blocked') {
    const bx = GX + l.pos * CW + 1;
    out.push(`<rect class="blink" x="${bx.toFixed(1)}" y="${y0 + 2.5}" width="${(CW - 2).toFixed(1)}" height="${3 * ROW_H + 3}" fill="none" stroke="${C.warn}"/>`);
  }
});
out.push('</svg>');

const file = path.join(__dirname, 'readme-banner.svg');
fs.writeFileSync(file, out.join('\n') + '\n');
console.log('wrote', path.relative(process.cwd(), file));
