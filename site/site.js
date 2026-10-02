// AI Agent Cockpit 宣傳頁：語言切換（預設英文，台港澳中自動繁中）＋主視覺點陣動畫。
(function () {
  'use strict';

  // ---------------- 語言（初始語言由 index.html head 腳本決定，記在 data-lang） ----------------
  var KEY = 'cockpit.site.lang';
  var dict = window.COCKPIT_I18N || {};
  // 原文（英文）記在元素上，切回英文時還原
  function apply(lang) {
    var zh = lang === 'zh';
    document.documentElement.lang = zh ? 'zh-Hant' : 'en';
    var nodes = document.querySelectorAll('[data-i18n]');
    for (var i = 0; i < nodes.length; i++) {
      var el = nodes[i], k = el.getAttribute('data-i18n'), attr = el.getAttribute('data-i18n-attr');
      if (attr) {
        if (!el.hasAttribute('data-en')) el.setAttribute('data-en', el.getAttribute(attr));
        el.setAttribute(attr, zh && dict[k] ? dict[k] : el.getAttribute('data-en'));
      } else {
        if (!el.hasAttribute('data-en')) el.setAttribute('data-en', el.innerHTML);
        // 字典是本檔自帶的靜態字串（含少量 <code>），不含外部輸入
        el.innerHTML = zh && dict[k] ? dict[k] : el.getAttribute('data-en');
      }
    }
    var btn = document.getElementById('lang-toggle');
    btn.textContent = zh ? 'English' : '中文';
    btn.setAttribute('lang', zh ? 'en' : 'zh-Hant');
    btn.setAttribute('aria-label', zh ? 'Switch to English' : '切換為繁體中文');
    document.documentElement.classList.remove('i18n-pending');
  }

  var current = document.documentElement.getAttribute('data-lang') === 'zh' ? 'zh' : 'en';
  apply(current);
  document.getElementById('lang-toggle').addEventListener('click', function () {
    current = current === 'zh' ? 'en' : 'zh';
    try { localStorage.setItem(KEY, current); } catch (e) { /* 隱私模式：只在本頁有效 */ }
    apply(current);
  });

  // ---------------- 主視覺：四條工作流的點陣訊號 ----------------
  var C = { line: '#294258', dim: '#a3b7c9', accent: '#63d5e8', ok: '#39d5ac', warn: '#e9bc73' };
  var reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var canvases = Array.prototype.slice.call(document.querySelectorAll('.dots-wrap canvas'));
  var labels = document.querySelectorAll('.lane-label .st');
  if (!canvases.length) return;

  function fit(cv) {
    var dpr = Math.min(window.devicePixelRatio || 1, 2), r = cv.getBoundingClientRect();
    cv.width = Math.max(1, Math.round(r.width * dpr)); cv.height = Math.max(1, Math.round(r.height * dpr));
    var ctx = cv.getContext('2d'); ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    return { ctx: ctx, w: r.width, h: r.height };
  }
  function rng(seed) { var s = seed >>> 0; return function () { s = (s * 1664525 + 1013904223) >>> 0; return s / 4294967296; }; }

  var STEP = 0.11, acc = 0, rnd = rng(42);
  var lanes = canvases.map(function (cv, i) { return { cv: cv, g: fit(cv), pos: [3, 11, 19, 6][i % 4], state: 'running', hold: 0, trail: [] }; });
  function cols(l) { return Math.max(18, Math.floor(l.g.w / 9)); }
  function setLabel(i, st) { var el = labels[i]; if (!el) return; el.textContent = st; el.className = 'st ' + (st === 'running' ? '' : st); }

  function tick() {
    lanes.forEach(function (l, i) {
      var n = cols(l), third = Math.floor(n / 3);
      if (l.state === 'blocked') { if (--l.hold <= 0) { l.state = 'running'; setLabel(i, 'running'); } return; }
      if (l.state === 'done') { if (--l.hold <= 0) { l.state = 'running'; l.pos = 0; l.trail = []; setLabel(i, 'running'); } return; }
      l.trail.push(l.pos); if (l.trail.length > 7) l.trail.shift();
      l.pos++;
      if ((l.pos === third || l.pos === third * 2) && rnd() < 0.45) { l.state = 'blocked'; l.hold = 14 + Math.floor(rnd() * 14); setLabel(i, 'blocked'); }
      if (l.pos >= n - 1) { l.state = 'done'; l.hold = 10; setLabel(i, 'completed'); }
    });
  }

  function render(now) {
    lanes.forEach(function (l) {
      var c = l.g.ctx, n = cols(l), w = l.g.w, h = l.g.h, cw = w / n, rows = 3, rh = h / rows, third = Math.floor(n / 3);
      c.clearRect(0, 0, w, h);
      for (var x = 0; x < n; x++) {
        for (var y = 0; y < rows; y++) {
          var col = C.line, a = 0.9, s = 2, t = l.trail.indexOf(x);
          if (x === l.pos && y === 1) { col = l.state === 'blocked' ? C.warn : l.state === 'done' ? C.ok : C.accent; s = 4; a = 1; }
          else if (x === l.pos) { col = l.state === 'blocked' ? C.warn : C.accent; s = 3; a = 0.55; }
          else if (t >= 0 && y === 1) { col = l.state === 'done' ? C.ok : C.accent; a = 0.15 + t * 0.1; s = 3; }
          else if (x < l.pos && y === 1) { col = C.ok; a = 0.5; }
          if ((x === third || x === third * 2) && col === C.line) { col = C.dim; a = 0.35; }
          c.globalAlpha = a; c.fillStyle = col;
          c.fillRect(Math.round(x * cw + cw / 2 - s / 2), Math.round(y * rh + rh / 2 - s / 2), s, s);
        }
      }
      if (l.state === 'blocked') {
        c.globalAlpha = reduced ? 1 : 0.5 + 0.5 * Math.sin(now / 160);
        c.strokeStyle = C.warn; c.lineWidth = 1; c.strokeRect(Math.round(l.pos * cw + 1), 1.5, Math.round(cw - 2), h - 3);
      }
      c.globalAlpha = 1;
    });
  }

  // 減少動態效果：一張靜止的畫面，仍看得出一條卡住、一條完成（縮放後依新欄數重放）
  function staticFrame() {
    lanes[2].pos = Math.min(19, Math.floor(cols(lanes[2]) * 2 / 3)); lanes[2].state = 'blocked'; setLabel(2, 'blocked');
    lanes[3].pos = cols(lanes[3]) - 1; lanes[3].state = 'done'; setLabel(3, 'completed');
  }
  window.addEventListener('resize', function () {
    lanes.forEach(function (l) { l.g = fit(l.cv); });
    if (reduced) staticFrame();
    render(performance.now());
  });

  if (reduced) { staticFrame(); render(0); return; }

  // 只在主視覺可見、分頁在前景時跑動畫
  var id = 0, last = 0, visible = true;
  function frame(t) {
    var dt = Math.max(0, Math.min(0.05, (t - last) / 1000)); last = t;
    acc += dt; while (acc >= STEP) { acc -= STEP; tick(); }
    render(t); id = requestAnimationFrame(frame);
  }
  function start() { if (id || document.hidden || !visible) return; last = performance.now(); id = requestAnimationFrame(frame); }
  function stop() { cancelAnimationFrame(id); id = 0; }
  document.addEventListener('visibilitychange', function () { if (document.hidden) stop(); else start(); });
  if ('IntersectionObserver' in window) {
    new IntersectionObserver(function (es) { visible = es[es.length - 1].isIntersecting; if (visible) start(); else stop(); })
      .observe(document.querySelector('.panel'));
  }
  start();
})();
