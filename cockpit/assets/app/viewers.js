// viewers.js：檔案分頁的檢視器（spec file-review「檔案檢視器」；design D5、D6、D8、D10、D11）。
// file-review task 4.1 建立模組骨架與路由（`/app/viewers.js`）；file-review task 4.4 填入 Markdown、純文字、
// HTML 與不支援四種檢視器；file-review task 4.5 補上 PDF（見下方「PDF」一節）。
//
// 對外只暴露 `window.cockpitViewers`（沿用 `window.liveOutput`／`window.cockpitActions` 的全域掛勾
// 慣例）、`window.cockpitMarkdownAnchor`（見下方「錨點」）與 `window.cockpitViewerHost`（`release(host)`、
// `state(host)`；見下方「PDF」一節的資源釋放與頁碼狀態）；files.js 開檔時才向它要檢視器，所以本檔在
// files.js 之前載入即可，不依賴任何其他模組。
//
// 檢視器入口（file-review task 4.3 定下，由 files.js 呼叫）：
//   `window.cockpitViewers[<中繼資料的 viewer>](ctx)`，viewer ∈ markdown｜text｜html｜pdf｜unsupported。
//   files.js 在檔案分頁成為目前分頁、取得中繼資料之後呼叫；沒有對應的函式時 files.js 顯示占位文字。
//   ctx：
//     host       檢視器容器（`.file-viewer-host`，也是內容的捲動容器；files.js 切換分頁時記下／寫回它的
//                scrollTop）。檢視器負責在裡面放一個帶 `data-viewer="<viewer>"` 的元素包住內容
//                （files-check 契約 C6），以 mount() 換掉先前的內容（保住捲動位置，見 mount()）。
//     file       { runtime, rootId, rootName, path }（path 為 `/` 分隔的相對路徑）
//     meta       中繼資料端點的回應（size、modified_ms、viewer、icon、vscode_uri）
//     rawUrl     原始內容端點網址；renderUrl：Markdown 渲染端點網址（兩者都已逐段編碼）
//     rawUrlOf(path)  同一個根目錄內另一個檔案的原始內容端點網址（Markdown 圖片用；file-review task 4.4）
//     openFile(path, anchor)  在分頁區開啟同一個根目錄內的另一個檔案；anchor（可省略）為開好後要捲到的
//                     標題錨點名稱（未加 `md-` 前綴、已解碼；Markdown 相對連結用）
//     isCurrent()     這次呼叫是否仍有效（分頁被關掉、被切走或開始下一次重讀之後回 false；晚到的結果應丟棄。
//                     自動更新的中繼資料輪詢本身不會讓它變成 false；file-review task 4.6）
//   回傳值（可為 Promise）：成功回 undefined 或 `{ ok: true }`，files.js 記下「最後一次成功讀取的時間」；
//   讀取失敗回 `{ ok: false, code }`（code 為錯誤本體的 `code`，連線失敗 "network"、逾時 "timeout"），
//   files.js 依 code 在狀態列顯示中文原因（design D10），分頁不消失。丟出例外視同 `{ ok: false, code: "unknown" }`。
//
// Markdown（design D5、D8）：渲染端點回的 HTML 片段先放進 `<template>`（其內容是惰性的：圖片不載入、腳本
// 不執行），在 template 裡依序做完三件事之後才移入頁面——
//   1. 白名單清洗（縱深防禦）：comrak 已把原始 HTML 換成註解、清掉危險 scheme（task 2.6 查證），這裡再以
//      元素／屬性白名單兜底：白名單外的元素換成它的文字、白名單外的屬性（含所有 on* 事件屬性、style）
//      一律拿掉。白名單＝comrak 在本專案選項下會輸出的元素與屬性（見 ALLOWED）。
//   2. 連結改寫（classifyHref）：`#錨點` → 同頁捲動；`http:`／`https:` → `target="_blank"`、
//      `rel="noopener noreferrer"`；不含 scheme 的相對路徑 → 以目前檔案所在資料夾為基準解析（resolveRelative），
//      在根目錄內 → 攔截點擊改為在分頁區開檔（可帶錨點）；其餘（其他 scheme、`//` 或 `\\` 開頭、跳出根目錄、
//      空的 href）→ 拿掉 href，點了不動作。
//   3. 圖片改寫：相對路徑且解析後在根目錄內 → 改為該檔的原始內容端點網址；`data:image/png|gif|jpeg|webp`
//      保留（不發請求；SVG 的 data URL 不保留——comrak 本身也會把它清成空字串）；其餘（絕對網址、`//`
//      或 `\\` 開頭、其他 scheme、跳出根目錄）→ 把 img 換成顯示 alt 的文字節點（不發任何請求）。
//   點擊以事件委派掛在檢視器元素上（依改寫時寫下的 data-md-* 屬性行動），整段內容換掉也不必重掛。
//
// 錨點：`window.cockpitMarkdownAnchor.reveal(host, name)` 在 host（捲動容器）內找 `md-` ＋ name 的標題並
// 捲到它（找不到時再試小寫，對應 comrak 的 anchorize 小寫化）。只動 host 的 scrollTop，不捲動頁面、不改
// location.hash。files.js 以它處理「開另一個檔並捲到錨點」（開好之後才捲）。

(function () {
  "use strict";

  var REQUEST_TIMEOUT_MS = 10000; // 同 files.js
  var TEXT_PREVIEW_MAX_BYTES = 2 * 1024 * 1024; // spec「檢視器」text：超過 2 MiB 不讀內容
  var HTML_LOAD_TIMEOUT_MS = 10000; // HTML iframe 等導覽結果的上限（R30）；同文字檔，逾時文案「超過 10 秒」才準
  // HTML iframe 導覽的 HTTP 狀態碼（Resource Timing 的 responseStatus）→ 錯誤 code（R30）。只有狀態碼、沒有錯誤
  // 本體可讀，同一個狀態碼有多種 code 時取原始內容讀取最可能的那一個；對不上（含 0：連線失敗、狀態碼不可得）
  // 一律 io_error。
  var HTML_STATUS_CODE = { 400: "bad_request", 403: "path_outside_root", 404: "not_found", 413: "too_large", 500: "io_error" };
  var ANCHOR_PREFIX = "md-"; // spec「Markdown 渲染端點」：標題 id 一律以 md- 開頭
  var ANCHOR_GAP_PX = 8; // 捲到錨點時標題上方留的空隙
  var SCHEME_RE = /^[a-z][a-z0-9+.\-]*:/i;
  var NETWORK_PATH_RE = /^[\/\\]{2}/; // `//host`、`\\host`、`/\host`、`\/host`：瀏覽器都當成另一台主機
  var RASTER_DATA_IMAGE_RE = /^data:image\/(?:png|gif|jpeg|webp)[;,]/i;
  var TOO_LARGE_TEXT = "檔案太大，無法預覽";
  var UNSUPPORTED_TEXT = "不支援預覽";
  var INERT_LINK_TITLE = "這個連結不會開啟（指向根目錄以外，或不是 http／https 網址）";
  var BLOCKED_IMAGE_TITLE = "這張圖片沒有載入（只載入根目錄內以相對路徑引用的圖片）";

  // --- 共用 ---

  // 一次 GET：resolve 成 `{ ok: true, body }` 或 `{ ok: false, code }`（不 reject）；body 依 kind 為字串（"text"）
  // 或 ArrayBuffer（"arrayBuffer"）。code 的來源同 files.js 的 getJson()（錯誤本體的 `code`；本體不是 JSON 為
  // "unknown"；連線失敗 "network"；逾時 "timeout"）。逾時涵蓋讀完整個本體。
  // maxBytes（可省略，只用於 "text"）：以實際收到的位元組套用上限（最終修正波 F2）——本體以串流讀取、邊讀邊累計，
  // 超過上限時立即取消讀取並 resolve 成 `{ ok: true, tooLarge: true }`，不看 Content-Length 或中繼資料的 size
  // （兩者都可能與實際內容不符，例如檔案在查中繼資料之後被改大）。
  function getBody(url, kind, timeoutMs, maxBytes) {
    var controller = new AbortController();
    var timedOut = false;
    var timer = setTimeout(function () {
      timedOut = true;
      controller.abort();
    }, timeoutMs);
    return fetch(url, { signal: controller.signal, cache: "no-store" })
      .then(function (response) {
        if (response.ok && typeof maxBytes === "number") {
          return readTextCapped(response, maxBytes);
        }
        if (response.ok) {
          return (kind === "arrayBuffer" ? response.arrayBuffer() : response.text()).then(function (body) {
            return { ok: true, body: body };
          });
        }
        return response.json().then(
          function (body) {
            return { ok: false, code: body && typeof body.code === "string" ? body.code : "unknown" };
          },
          function () {
            return { ok: false, code: "unknown" };
          }
        );
      })
      .catch(function () {
        return { ok: false, code: timedOut ? "timeout" : "network" };
      })
      .then(function (result) {
        clearTimeout(timer);
        return result;
      });
  }

  // 串流讀本體並以 UTF-8 解碼（無效位元組以替代字元處理、去掉開頭 BOM，同 response.text()），累計位元組超過
  // maxBytes 時取消讀取（reader.cancel() 中止底層連線）並回 `{ ok: true, tooLarge: true }`。
  function readTextCapped(response, maxBytes) {
    var reader = response.body.getReader();
    var decoder = new TextDecoder("utf-8");
    var total = 0;
    var parts = [];
    function pump() {
      return reader.read().then(function (step) {
        if (step.done) {
          parts.push(decoder.decode());
          return { ok: true, body: parts.join("") };
        }
        total += step.value.byteLength;
        if (total > maxBytes) {
          reader.cancel().catch(function () {});
          return { ok: true, tooLarge: true };
        }
        parts.push(decoder.decode(step.value, { stream: true }));
        return pump();
      });
    }
    return pump();
  }

  // 一次 GET 文字：`{ ok: true, text }`、`{ ok: true, tooLarge: true }`（只在給了 maxBytes 時）或 `{ ok: false, code }`。
  function getText(url, maxBytes) {
    return getBody(url, "text", REQUEST_TIMEOUT_MS, maxBytes).then(function (result) {
      return result.ok && !result.tooLarge ? { ok: true, text: result.body } : result;
    });
  }

  function el(tag, className) {
    var node = document.createElement(tag);
    if (className) {
      node.className = className;
    }
    return node;
  }

  // 檢視器容器 → 目前掛在上面、需要釋放資源的檢視器工作階段（目前只有 PDF：`{ viewer, state(), release() }`），
  // 以及還在讀取中的 PDF 載入（`{ destroy() }`）。見下方「檢視器資源的釋放」。
  var hostSessions = new WeakMap();
  var pendingLoads = new WeakMap();

  function releaseSession(host) {
    var session = hostSessions.get(host);
    if (session !== undefined) {
      hostSessions.delete(host);
      session.release();
    }
  }

  // 把檢視器元素放進 host，換掉先前的內容；host 的捲動位置在換內容前後保持不變（自動更新重讀同一個檔時
  // 不跳回頂端；file-review task 4.6 沿用）。先前的內容若是 PDF，先釋放它的文件與 worker（file-review task 4.5）。
  function mount(ctx, node) {
    var host = ctx.host;
    var top = host.scrollTop;
    releaseSession(host);
    host.replaceChildren(node);
    if (top > 0) {
      host.scrollTop = top;
    }
  }

  // 人類可讀的檔案大小（1024 進位）。
  function formatSize(bytes) {
    if (typeof bytes !== "number" || !isFinite(bytes) || bytes < 0) {
      return "未知";
    }
    if (bytes < 1024) {
      return bytes + " B";
    }
    var units = ["KB", "MB", "GB", "TB"];
    var value = bytes / 1024;
    var i = 0;
    while (value >= 1024 && i < units.length - 1) {
      value /= 1024;
      i += 1;
    }
    return (value < 10 ? value.toFixed(1) : String(Math.round(value))) + " " + units[i];
  }

  // 「不支援預覽」「檔案太大，無法預覽」共用的說明區塊：一行說明＋檔案大小（數值用等寬，design D11）。
  function noteView(viewer, title, size) {
    var box = el("div", "viewer-note");
    box.setAttribute("data-viewer", viewer);
    var head = el("p", "viewer-note-title");
    head.textContent = title;
    var meta = el("p", "viewer-note-meta");
    meta.appendChild(document.createTextNode("檔案大小："));
    var value = el("span", "viewer-note-size");
    value.textContent = formatSize(size);
    if (typeof size === "number") {
      value.title = size + " 位元組";
    }
    meta.appendChild(value);
    box.appendChild(head);
    box.appendChild(meta);
    return box;
  }

  // --- Markdown：網址分類與相對路徑解析（design D8）---

  // 瀏覽器解析網址前會做的清理：拿掉所有 tab／換行、去掉頭尾的控制字元與空白（`java\tscript:` 之類的
  // 寫法因此跟瀏覽器看到的一樣被歸類）。
  function cleanUrl(raw) {
    return String(raw)
      .replace(/[\t\n\r]/g, "")
      .replace(/^[\u0000- ]+|[\u0000- ]+$/g, "");
  }

  function decodePart(s) {
    try {
      return decodeURIComponent(s);
    } catch (e) {
      return null;
    }
  }

  // 以目前檔案所在資料夾（dirSegs）為基準解析不含 scheme 的相對參照，回傳 `{ path, anchor }` 或 null。
  // 自己逐段處理 `.`／`..`，不用 URL 物件：URL 會把跳出根目錄的 `..` 夾在根上（`../../x` 從 `docs/`
  // 解析成 `/x`），看不出「跳出根目錄」。null＝跳出根目錄、指向資料夾或根目錄本身、或含無法處理的片段
  // （解碼失敗、解碼後含 `/` 或 `\`）。以 `/` 開頭的路徑以根目錄為基準；`?` 之後的查詢字串忽略。
  function resolveRelative(ref, dirSegs) {
    var hashAt = ref.indexOf("#");
    var anchor = null;
    if (hashAt >= 0) {
      anchor = decodePart(ref.slice(hashAt + 1));
      if (anchor === null) {
        return null;
      }
    }
    var pathPart = hashAt >= 0 ? ref.slice(0, hashAt) : ref;
    var queryAt = pathPart.indexOf("?");
    if (queryAt >= 0) {
      pathPart = pathPart.slice(0, queryAt);
    }
    if (pathPart === "" || /[\/\\]$/.test(pathPart)) {
      return null;
    }
    var stack = /^[\/\\]/.test(pathPart) ? [] : dirSegs.slice();
    var parts = pathPart.split(/[\/\\]/);
    for (var i = 0; i < parts.length; i += 1) {
      var seg = decodePart(parts[i]);
      if (seg === null || /[\/\\]/.test(seg)) {
        return null;
      }
      if (seg === "" || seg === ".") {
        continue;
      }
      if (seg === "..") {
        if (stack.length === 0) {
          return null; // 跳出根目錄
        }
        stack.pop();
        continue;
      }
      stack.push(seg);
    }
    if (stack.length === 0) {
      return null;
    }
    return { path: stack.join("/"), anchor: anchor === "" ? null : anchor };
  }

  // a[href] 的分類：{ kind: "anchor", anchor }｜{ kind: "external" }｜{ kind: "file", path, anchor }｜
  // { kind: "inert" }。
  function classifyHref(raw, dirSegs) {
    var s = cleanUrl(raw);
    if (s === "") {
      return { kind: "inert" };
    }
    if (s.charAt(0) === "#") {
      var anchor = decodePart(s.slice(1));
      return anchor ? { kind: "anchor", anchor: anchor } : { kind: "inert" };
    }
    if (NETWORK_PATH_RE.test(s)) {
      return { kind: "inert" };
    }
    var scheme = SCHEME_RE.exec(s);
    if (scheme !== null) {
      var name = scheme[0].toLowerCase();
      return name === "http:" || name === "https:" ? { kind: "external" } : { kind: "inert" };
    }
    var resolved = resolveRelative(s, dirSegs);
    return resolved === null ? { kind: "inert" } : { kind: "file", path: resolved.path, anchor: resolved.anchor };
  }

  // img[src] 改寫後的網址；null＝不載入（換成 alt 文字）。
  function imageSrc(raw, dirSegs, ctx) {
    var s = cleanUrl(raw);
    if (RASTER_DATA_IMAGE_RE.test(s)) {
      return s;
    }
    if (s === "" || s.charAt(0) === "#" || NETWORK_PATH_RE.test(s) || SCHEME_RE.test(s)) {
      return null;
    }
    var resolved = resolveRelative(s, dirSegs);
    return resolved === null ? null : ctx.rawUrlOf(resolved.path);
  }

  // --- Markdown：白名單清洗（縱深防禦，見檔頭）---

  // 元素 → 允許的屬性。comrak（本專案選項：table、tasklist、strikethrough、autolink、header_id_prefix）
  // 會輸出的就這些；其餘元素換成它的文字。
  var HEADING_ATTRS = ["id"];
  var ALLOWED = {
    H1: HEADING_ATTRS,
    H2: HEADING_ATTRS,
    H3: HEADING_ATTRS,
    H4: HEADING_ATTRS,
    H5: HEADING_ATTRS,
    H6: HEADING_ATTRS,
    P: [],
    A: ["href", "title", "class"],
    IMG: ["src", "alt", "title"],
    UL: [],
    OL: ["start"],
    LI: [],
    INPUT: ["type", "checked", "disabled"],
    TABLE: [],
    THEAD: [],
    TBODY: [],
    TR: [],
    TH: ["align"],
    TD: ["align"],
    CODE: ["class"],
    PRE: [],
    EM: [],
    STRONG: [],
    DEL: [],
    BLOCKQUOTE: [],
    HR: [],
    BR: [],
  };
  var ALIGN_VALUES = { left: true, center: true, right: true };

  function attrValueOk(tag, name, value) {
    if (name === "id") {
      return value.indexOf(ANCHOR_PREFIX) === 0;
    }
    if (name === "class") {
      return tag === "A" ? value === "anchor" : /^language-[\w+#.\-]+$/.test(value);
    }
    if (name === "align") {
      return ALIGN_VALUES[value] === true;
    }
    if (name === "type") {
      return value === "checkbox";
    }
    if (name === "start") {
      return /^\d+$/.test(value);
    }
    return true;
  }

  function sanitize(fragment) {
    var nodes = fragment.querySelectorAll("*");
    for (var i = 0; i < nodes.length; i += 1) {
      var node = nodes[i];
      var allowed = Object.prototype.hasOwnProperty.call(ALLOWED, node.tagName) ? ALLOWED[node.tagName] : null;
      if (allowed === null || (node.tagName === "INPUT" && node.getAttribute("type") !== "checkbox")) {
        if (node.parentNode !== null) {
          node.parentNode.replaceChild(document.createTextNode(node.textContent), node);
        }
        continue;
      }
      var attrs = Array.prototype.slice.call(node.attributes);
      for (var j = 0; j < attrs.length; j += 1) {
        var name = attrs[j].name.toLowerCase();
        if (allowed.indexOf(name) < 0 || !attrValueOk(node.tagName, name, attrs[j].value)) {
          node.removeAttribute(attrs[j].name);
        }
      }
      if (node.tagName === "INPUT") {
        node.disabled = true; // 任務清單的勾選框只顯示狀態
      }
    }
  }

  // --- Markdown：連結與圖片改寫（design D8，在 template 內、移入頁面之前）---

  function rewriteLinks(fragment, dirSegs) {
    var links = fragment.querySelectorAll("a");
    for (var i = 0; i < links.length; i += 1) {
      var a = links[i];
      var info = a.hasAttribute("href") ? classifyHref(a.getAttribute("href"), dirSegs) : { kind: "inert" };
      a.setAttribute("data-md-link", info.kind);
      if (info.kind === "external") {
        a.setAttribute("target", "_blank");
        a.setAttribute("rel", "noopener noreferrer");
      } else if (info.kind === "anchor") {
        a.setAttribute("data-md-anchor", info.anchor);
      } else if (info.kind === "file") {
        a.setAttribute("data-md-path", info.path);
        if (info.anchor !== null) {
          a.setAttribute("data-md-anchor", info.anchor);
        }
      } else {
        a.removeAttribute("href");
        a.title = INERT_LINK_TITLE;
      }
      // comrak 在每個標題後放一個空的 `a.anchor`（href 是未加前綴的標題 id），aria-label 是英文；
      // 改成中文，鍵盤聚焦時才看得到「#」（見 style.css .md-body .anchor）。
      if (a.classList.contains("anchor")) {
        var heading = a.closest("h1,h2,h3,h4,h5,h6");
        a.setAttribute("aria-label", "連到標題「" + (heading ? heading.textContent.trim() : "") + "」");
      }
    }
  }

  function rewriteImages(fragment, dirSegs, ctx) {
    var images = fragment.querySelectorAll("img");
    for (var i = 0; i < images.length; i += 1) {
      var img = images[i];
      var src = img.hasAttribute("src") ? imageSrc(img.getAttribute("src"), dirSegs, ctx) : null;
      if (src === null) {
        var alt = el("span", "md-img-alt");
        alt.textContent = img.getAttribute("alt") || "圖片";
        alt.title = BLOCKED_IMAGE_TITLE;
        img.parentNode.replaceChild(alt, img);
      } else {
        img.setAttribute("src", src);
        img.setAttribute("decoding", "async");
      }
    }
  }

  // --- 錨點 ---

  function findAnchor(root, name) {
    if (typeof name !== "string" || name === "" || typeof CSS === "undefined" || typeof CSS.escape !== "function") {
      return null;
    }
    var target = root.querySelector("#" + CSS.escape(ANCHOR_PREFIX + name));
    if (target === null && name.toLowerCase() !== name) {
      target = root.querySelector("#" + CSS.escape(ANCHOR_PREFIX + name.toLowerCase()));
    }
    return target;
  }

  // 在 host 內捲到錨點；回傳是否找到（找不到時不捲動）。
  function revealAnchor(host, name) {
    var view = host.querySelector('[data-viewer="markdown"]');
    var target = view !== null ? findAnchor(view, name) : null;
    if (target === null) {
      return false;
    }
    var offset = target.getBoundingClientRect().top - host.getBoundingClientRect().top;
    host.scrollTop = Math.max(0, host.scrollTop + offset - ANCHOR_GAP_PX);
    return true;
  }

  // --- 檢視器 ---

  function dirOf(path) {
    var parts = path.split("/");
    parts.pop();
    return parts;
  }

  function markdownViewer(ctx) {
    return getText(ctx.renderUrl).then(function (result) {
      if (!ctx.isCurrent()) {
        return undefined;
      }
      if (!result.ok) {
        return { ok: false, code: result.code };
      }
      var dirSegs = dirOf(ctx.file.path);
      var template = document.createElement("template");
      template.innerHTML = result.text; // 惰性：此時沒有任何請求、腳本或事件處理器生效
      sanitize(template.content);
      rewriteLinks(template.content, dirSegs);
      rewriteImages(template.content, dirSegs, ctx);
      var view = el("div", "md-body");
      view.setAttribute("data-viewer", "markdown");
      view.appendChild(template.content); // 改寫完才移出 template（design D5）
      view.addEventListener("click", function (event) {
        var a = event.target instanceof Element ? event.target.closest("a") : null;
        if (a === null || !view.contains(a) || a.getAttribute("data-md-link") === "external") {
          return;
        }
        event.preventDefault();
        var kind = a.getAttribute("data-md-link");
        if (kind === "anchor") {
          revealAnchor(ctx.host, a.getAttribute("data-md-anchor"));
        } else if (kind === "file") {
          ctx.openFile(a.getAttribute("data-md-path"), a.getAttribute("data-md-anchor"));
        }
      });
      // 中鍵等輔助按鍵不在新分頁開內部連結（那些 href 只對分頁區有意義）。
      view.addEventListener("auxclick", function (event) {
        var a = event.target instanceof Element ? event.target.closest("a") : null;
        if (a !== null && view.contains(a) && a.getAttribute("data-md-link") !== "external") {
          event.preventDefault();
        }
      });
      mount(ctx, view);
      return { ok: true };
    });
  }

  // 純文字：行號在左（sticky，橫向捲動時留在原地），內容一律以 textContent 放入（不被解讀為 HTML）。
  // 2 MiB 上限（spec「檢視器」text）：中繼資料的 size 已超過就不發請求；否則以實際收到的位元組再套用一次
  // （最終修正波 F2：查中繼資料之後檔案可能被改大），超過時中止讀取、顯示同一個「檔案太大」說明。
  function textViewer(ctx) {
    var size = ctx.meta ? ctx.meta.size : undefined;
    if (typeof size === "number" && size > TEXT_PREVIEW_MAX_BYTES) {
      mount(ctx, noteView("text", TOO_LARGE_TEXT, size));
      return { ok: true };
    }
    return getText(ctx.rawUrl, TEXT_PREVIEW_MAX_BYTES).then(function (result) {
      if (!ctx.isCurrent()) {
        return undefined;
      }
      if (!result.ok) {
        return { ok: false, code: result.code };
      }
      if (result.tooLarge) {
        mount(ctx, noteView("text", TOO_LARGE_TEXT, undefined));
        return { ok: true };
      }
      var lines = result.text.replace(/\r\n?/g, "\n").split("\n");
      if (lines.length > 1 && lines[lines.length - 1] === "") {
        lines.pop(); // 結尾換行不多算一行
      }
      var numbers = new Array(lines.length);
      for (var i = 0; i < lines.length; i += 1) {
        numbers[i] = String(i + 1);
      }
      var view = el("div", "text-viewer");
      view.setAttribute("data-viewer", "text");
      var gutter = el("pre", "text-gutter");
      gutter.setAttribute("aria-hidden", "true");
      gutter.textContent = numbers.join("\n");
      var content = el("pre", "text-content");
      content.textContent = lines.join("\n");
      view.appendChild(gutter);
      view.appendChild(content);
      mount(ctx, view);
      return { ok: true };
    });
  }

  // HTML：sandbox iframe 直接載入原始內容端點（design D8：不做改寫，相對引用自然落在同一個端點下；不改用
  // srcdoc——同一個根目錄內以相對路徑引用的樣式表與圖片要靠端點網址解析）。sandbox 屬性值是空字串（不含
  // allow-scripts 與 allow-same-origin，也不給表單、彈出視窗、頂層導覽）；先設 sandbox 再設 src。
  // 讀取成敗的判斷（最終修正波 F3／控制端裁決 R30，取代 R28 的預先 fetch）：iframe 的 load 事件分不出 HTTP
  // 成功與否（錯誤本體也會「載入成功」），而 sandbox iframe 的文件是不透明來源、讀不到內容，所以改看父頁的
  // Resource Timing——iframe 導覽會在父頁留下一筆 initiatorType "iframe" 的 entry，其 responseStatus 是這次
  // 導覽的 HTTP 狀態碼（同源網址。Chrome 實測：服務回的 200／400／404 與 CDP 在回應階段改寫的 500／503 都如實
  // 反映；CDP 直接合成的回應與連線失敗為 0；entry 在 load 事件當下已可取得，隱藏的 iframe 也一樣）。流程：
  //   1. 新 iframe 包在隱藏（hidden）的新檢視器元素裡插入 host，舊內容照常顯示。src＝原始內容端點＋唯一的
  //      `?_cv=<序號>`（服務端只看路徑、忽略 query），讓 entry 的 name 對得上這一次導覽。
  //   2. 以 PerformanceObserver 收 entry（不受 resource timing buffer 滿了的影響；輪詢會很快填滿 250 筆的預設
  //      buffer，之後 performance.getEntriesByName() 就找不到新的 entry）。
  //   3. 收到這一次的 entry（導覽回應收完就會送達，不等頁內的圖片等子資源——實測頁內引用一個連不上的外部
  //      圖片時，entry 在 17 ms 送達、load 要 23 秒）或 load（當下以 takeRecords() 補收）時判斷：responseStatus
  //      為 200 → 移除舊內容、顯示新 iframe（不搬動它，搬動 iframe 會重新載入），回 ok；不是 200、找不到 entry
  //      （CDP 合成的回應、連線失敗時為 0）或 HTML_LOAD_TIMEOUT_MS 內兩者都沒有 → 移除新 iframe、保留舊內容，
  //      回 `{ ok: false, code }`（files.js 標過期、顯示原因；ft.shown 沒變，同一個版本下一輪輪詢重試）。
  var htmlLoadSeq = 0;

  function htmlViewer(ctx) {
    htmlLoadSeq += 1;
    var src = ctx.rawUrl + "?_cv=" + htmlLoadSeq;
    var entryName = new URL(src, document.baseURI).href;
    var view = el("div", "html-viewer");
    view.setAttribute("data-viewer", "html");
    view.hidden = true;
    var frame = el("iframe", "html-frame");
    frame.setAttribute("sandbox", "");
    frame.title = ctx.file.path + " 的內容（腳本已停用）";
    view.appendChild(frame);
    return new Promise(function (resolve) {
      var statuses = [];
      var done = false;
      var timer = null;
      var collect = function (entries) {
        for (var i = 0; i < entries.length; i += 1) {
          if (entries[i].name === entryName && entries[i].initiatorType === "iframe") {
            statuses.push(entries[i].responseStatus);
          }
        }
      };
      var observer = new PerformanceObserver(function (list) {
        collect(list.getEntries());
        if (statuses.length > 0) {
          finish(false);
        }
      });
      var finish = function (timedOut) {
        if (done) {
          return;
        }
        done = true;
        clearTimeout(timer);
        collect(observer.takeRecords());
        observer.disconnect();
        if (!ctx.isCurrent()) {
          view.remove();
          resolve(undefined);
          return;
        }
        var status = statuses.length > 0 ? statuses[statuses.length - 1] : 0;
        if (status === 200) {
          reveal(ctx, view);
          resolve({ ok: true });
          return;
        }
        view.remove();
        resolve({ ok: false, code: statuses.length === 0 && timedOut ? "timeout" : HTML_STATUS_CODE[status] || "io_error" });
      };
      observer.observe({ type: "resource" });
      timer = setTimeout(function () {
        finish(true);
      }, HTML_LOAD_TIMEOUT_MS);
      frame.addEventListener(
        "load",
        function () {
          finish(false);
        },
        { once: true }
      );
      frame.src = src;
      ctx.host.appendChild(view);
    });
  }

  // 把已在 host 內（隱藏中）載入好的檢視器元素換成可見的內容：移除 host 內其他所有節點（含舊內容），不搬動
  // node 本身（搬動 iframe 會重新載入）；捲動位置與 PDF 資源的釋放同 mount()。
  function reveal(ctx, node) {
    var host = ctx.host;
    var top = host.scrollTop;
    releaseSession(host);
    var children = Array.prototype.slice.call(host.childNodes);
    for (var i = 0; i < children.length; i += 1) {
      if (children[i] !== node) {
        host.removeChild(children[i]);
      }
    }
    node.hidden = false;
    if (top > 0) {
      host.scrollTop = top;
    }
  }

  function unsupportedViewer(ctx) {
    mount(ctx, noteView("unsupported", UNSUPPORTED_TEXT, ctx.meta ? ctx.meta.size : undefined));
    return { ok: true };
  }

  // --- PDF（design D7；file-review task 4.5）---
  //
  // 載入：第一次開 PDF 才 `import()` vendored 的 pdf.js 6.3.289（函式庫模式，不用 viewer）。原始內容端點由本檔以
  // fetch 整檔讀成 ArrayBuffer 再交給 `getDocument({ data })`：讀取失敗時拿得到錯誤本體的 `code`（design D10；
  // pdf.js 自己讀網址只給 HTTP 狀態碼），也不會發 Range 請求。`disableRange`／`disableStream` 照 D7 仍一併設定。
  // 資源目錄（cmaps、standard_fonts、wasm、iccs）一律指向本服務的 `/vendor/pdfjs/`，不對外部網域發請求；選項
  // 名稱以 6.3.289 的 `src/display/api.js` 為準（查證見 cockpit/assets/vendor/README.md）。
  //
  // 版面：`.pdf-viewer`（data-viewer="pdf"）＝工具列＋頁面捲動區 `.pdf-pages`。每頁一個依頁面比例定尺寸的
  // `.pdf-page` 框，裡面恆有一個 `<canvas>`（files-check 契約 C6）；IntersectionObserver 在頁面接近可視範圍
  // （上下各一個捲動區高度）時才畫，離開時取消還沒畫完的 render task。畫在新的 canvas 上、畫完才換上（縮放時
  // 舊內容先被拉伸顯示，不閃白）。canvas 解析度考慮 devicePixelRatio，單張上限 PDF_MAX_CANVAS_PIXELS。已畫好
  // 的頁面最多保留 PDF_RENDERED_PAGES_MAX 張（同 pdf.js viewer 的頁面緩衝），超過時丟掉離目前頁最遠的一張。
  // 不做文字層（D7）。
  //
  // 縮放：符合寬度（預設；依捲動區寬度與最寬的一頁算）與 50%～300%、每次 25%。100%＝PDF 的 1 點等於
  // 96/72 CSS 像素（pdf.js 的 PixelsPerInch.PDF_TO_CSS_UNITS）。縮放、符合寬度下的容器改寬、分頁從隱藏
  // 變回可見時，都以「位置錨點」（頁碼＋在該頁內的相對位置）捲回原處，所以縮放後維持目前頁。
  //
  // 目前頁：捲動時取可視高度最大的一頁（同高取前面的）。上一頁／下一頁把該頁頂端捲到捲動區頂端，並直接把
  // 目前頁設成該頁（最後幾頁放得下時捲不動，仍要顯示目標頁碼）。
  //
  // 頁碼狀態（給 file-review task 4.6 的自動更新）：`window.cockpitViewerHost.state(host)` 回
  //   `{ viewer: "pdf", page, numPages, zoom, anchor: { page, offset } }`
  //   page＝工具列顯示的目前頁（從 1 起）；zoom＝"fit" 或 0.5～3 的倍率；anchor＝捲動區頂端所在的頁與在該頁內的
  //   相對位置（0～1）。同一個 host 再呼叫一次 pdf 檢視器（重讀同一個檔）時，新文件畫好才換掉舊內容，並自動
  //   沿用舊工作階段的 zoom 與 anchor（頁碼超出新總頁數時取最後一頁、頁內位置歸零）。
  //
  // 檢視器資源的釋放：換內容（mount）、`window.cockpitViewerHost.release(host)`（files.js 關閉分頁時呼叫）會
  // 取消所有 render task、中止還在讀的載入、`loadingTask.destroy()`（連帶銷毀文件與它的 worker）。同一個 host
  // 上較晚開始的載入會取代較早的那一個（較早的立刻 destroy）。
  //
  // 無法解析（getDocument 或取頁失敗）：顯示「PDF 無法解析」與檔案大小；這是讀到了檔案、內容不是可用的 PDF，
  // 所以算一次成功讀取（回 `{ ok: true }`）。原始內容讀不到才回 `{ ok: false, code }`。

  var PDF_BASE = "/vendor/pdfjs/";
  var PDF_REQUEST_TIMEOUT_MS = 30000; // 原始內容上限 50 MiB（spec「原始內容端點」），比文字檔寬鬆
  var PDF_ZOOM_MIN = 0.5;
  var PDF_ZOOM_MAX = 3;
  var PDF_ZOOM_STEP = 0.25;
  var PDF_ZOOM_EPSILON = 0.001;
  var PDF_PAD_PX = 12; // 同 style.css .pdf-pages 的 padding：符合寬度時左右各扣這麼多
  var PDF_MAX_CANVAS_PIXELS = 16777216; // 2^24 個像素（約 64 MB）
  var PDF_RENDERED_PAGES_MAX = 10;
  var PDF_NEAR_MARGIN = "100% 0px"; // IntersectionObserver：捲動區上下各一個自身高度內算「接近」
  var PDF_UNPARSABLE_TEXT = "PDF 無法解析";

  var pdfjsPromise = null;

  // 延遲載入 pdf.js（只載一次；失敗時下次再試）。
  function loadPdfjs() {
    if (pdfjsPromise === null) {
      pdfjsPromise = import(PDF_BASE + "pdf.min.mjs").then(function (lib) {
        lib.GlobalWorkerOptions.workerSrc = PDF_BASE + "pdf.worker.min.mjs";
        return lib;
      });
      pdfjsPromise.catch(function () {
        pdfjsPromise = null;
      });
    }
    return pdfjsPromise;
  }

  function pdfDocumentParams(bytes) {
    return {
      data: bytes,
      cMapUrl: PDF_BASE + "cmaps/",
      cMapPacked: true,
      standardFontDataUrl: PDF_BASE + "standard_fonts/",
      wasmUrl: PDF_BASE + "wasm/",
      iccUrl: PDF_BASE + "iccs/",
      disableRange: true,
      disableStream: true,
    };
  }

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  // 還沒畫（或已丟掉）的頁面放的 canvas：0×0（預設的 300×150 也要佔點陣圖記憶體，且看起來像「已畫」）。
  function blankCanvas() {
    var canvas = document.createElement("canvas");
    canvas.width = 0;
    canvas.height = 0;
    return canvas;
  }

  function quietly(promise) {
    if (promise && typeof promise.catch === "function") {
      promise.catch(function () {});
    }
  }

  // 一份已載入的 PDF 的畫面與狀態。carry：沿用的舊狀態（見檔頭「頁碼狀態」），可為 null。
  function createPdfSession(lib, loadingTask, proxies, carry) {
    var cssUnits = lib.PixelsPerInch && lib.PixelsPerInch.PDF_TO_CSS_UNITS ? lib.PixelsPerInch.PDF_TO_CSS_UNITS : 96 / 72;
    var numPages = proxies.length;
    var released = false;
    var laidOut = false;
    var zoomUsed = null; // 目前版面實際用的倍率
    var lastWidth = -1;
    var programmaticTop = null; // 自己設的 scrollTop：對應的 scroll 事件不重算目前頁
    var rafPending = false;
    var state = { page: 1, zoom: "fit", anchor: { page: 1, offset: 0 } };
    if (carry !== null && typeof carry === "object") {
      if (carry.zoom === "fit" || (typeof carry.zoom === "number" && carry.zoom >= PDF_ZOOM_MIN && carry.zoom <= PDF_ZOOM_MAX)) {
        state.zoom = carry.zoom;
      }
      var a = carry.anchor || { page: carry.page, offset: 0 };
      if (typeof a.page === "number" && a.page >= 1) {
        var outOfRange = a.page > numPages;
        state.anchor = {
          page: outOfRange ? numPages : Math.floor(a.page),
          offset: outOfRange || typeof a.offset !== "number" ? 0 : clamp(a.offset, 0, 1),
        };
      }
      if (typeof carry.page === "number" && carry.page >= 1) {
        state.page = Math.min(numPages, Math.floor(carry.page));
      }
    }

    var view = el("div", "pdf-viewer");
    view.setAttribute("data-viewer", "pdf");

    var bar = el("div", "pdf-toolbar");
    bar.setAttribute("role", "group");
    bar.setAttribute("aria-label", "PDF 頁面與縮放");
    function tool(text, onActivate) {
      var button = el("button", "action-button pdf-tool");
      button.type = "button";
      button.textContent = text;
      button.addEventListener("click", function () {
        if (button.getAttribute("aria-disabled") !== "true") {
          onActivate();
        }
      });
      return button;
    }
    var prevButton = tool("上一頁", function () {
      goToPage(state.page - 1);
    });
    var pageStatus = el("span", "pdf-page-status");
    pageStatus.title = "目前頁／總頁數";
    var nextButton = tool("下一頁", function () {
      goToPage(state.page + 1);
    });
    var zoomOutButton = tool("縮小", function () {
      stepZoom(-1);
    });
    var zoomLevel = el("span", "pdf-zoom-level");
    zoomLevel.title = "縮放比例";
    var zoomInButton = tool("放大", function () {
      stepZoom(1);
    });
    var fitButton = tool("符合寬度", function () {
      setZoom("fit");
    });
    var navGroup = el("span", "pdf-toolbar-group");
    navGroup.appendChild(prevButton);
    navGroup.appendChild(pageStatus);
    navGroup.appendChild(nextButton);
    var zoomGroup = el("span", "pdf-toolbar-group");
    zoomGroup.appendChild(zoomOutButton);
    zoomGroup.appendChild(zoomLevel);
    zoomGroup.appendChild(zoomInButton);
    zoomGroup.appendChild(fitButton);
    bar.appendChild(navGroup);
    bar.appendChild(zoomGroup);

    var scroller = el("div", "pdf-pages");
    var pages = proxies.map(function (proxy, i) {
      var vp = proxy.getViewport({ scale: 1 });
      var box = el("div", "pdf-page");
      box.setAttribute("data-page", String(i + 1));
      box.setAttribute("role", "img");
      box.setAttribute("aria-label", "第 " + (i + 1) + " 頁");
      var canvas = blankCanvas();
      box.appendChild(canvas);
      scroller.appendChild(box);
      return { num: i + 1, proxy: proxy, w: vp.width, h: vp.height, box: box, canvas: canvas, cssW: 0, cssH: 0, key: null, task: null, taskKey: null, near: false };
    });
    var maxPageWidth = pages.reduce(function (m, p) {
      return Math.max(m, p.w);
    }, 1);

    view.appendChild(bar);
    view.appendChild(scroller);

    // --- 縮放與版面 ---

    function fitZoom() {
      var avail = scroller.clientWidth - 2 * PDF_PAD_PX;
      return avail > 0 ? avail / (maxPageWidth * cssUnits) : null;
    }

    function currentZoom() {
      if (state.zoom !== "fit") {
        return state.zoom;
      }
      var fit = fitZoom();
      return fit !== null ? fit : zoomUsed;
    }

    // 依目前倍率設定每頁框的尺寸；回傳是否排得出來（捲動區寬度為 0 時不排）。
    function layout() {
      var zoom = currentZoom();
      if (zoom === null || !(zoom > 0)) {
        return false;
      }
      zoomUsed = zoom;
      pages.forEach(function (p) {
        p.cssW = Math.max(1, Math.round(p.w * zoom * cssUnits));
        p.cssH = Math.max(1, Math.round(p.h * zoom * cssUnits));
        p.box.style.width = p.cssW + "px";
        p.box.style.height = p.cssH + "px";
      });
      lastWidth = scroller.clientWidth;
      if (!laidOut) {
        laidOut = true;
        pages.forEach(function (p) {
          observer.observe(p.box);
        });
      }
      pages.forEach(function (p) {
        if (p.near) {
          renderPage(p);
        }
      });
      updateToolbar();
      return true;
    }

    function setScrollTop(top) {
      scroller.scrollTop = Math.max(0, top);
      programmaticTop = scroller.scrollTop;
    }

    // 捲動區頂端所在的頁與在該頁內的相對位置。
    function readAnchor() {
      var y = scroller.scrollTop + PDF_PAD_PX;
      var found = pages[0];
      for (var i = 0; i < pages.length; i += 1) {
        if (pages[i].box.offsetTop <= y) {
          found = pages[i];
        } else {
          break;
        }
      }
      var height = found.box.offsetHeight || 1;
      return { page: found.num, offset: clamp((y - found.box.offsetTop) / height, 0, 1) };
    }

    function applyAnchor() {
      var p = pages[clamp(state.anchor.page, 1, numPages) - 1];
      setScrollTop(p.box.offsetTop + state.anchor.offset * p.box.offsetHeight - PDF_PAD_PX);
    }

    // 可視高度最大的一頁（同高取前面的）。
    function mostVisiblePage() {
      var top = scroller.scrollTop;
      var bottom = top + scroller.clientHeight;
      var best = state.page;
      var bestVisible = -1;
      for (var i = 0; i < pages.length; i += 1) {
        var pt = pages[i].box.offsetTop;
        if (pt > bottom) {
          break;
        }
        var visible = Math.min(pt + pages[i].box.offsetHeight, bottom) - Math.max(pt, top);
        if (visible > bestVisible + 0.5) {
          best = pages[i].num;
          bestVisible = visible;
        }
      }
      return best;
    }

    function setZoom(zoom) {
      if (!laidOut) {
        state.zoom = zoom;
        return;
      }
      var centerX = scroller.scrollWidth > 0 ? (scroller.scrollLeft + scroller.clientWidth / 2) / scroller.scrollWidth : 0.5;
      // 維持目前頁：捲動區頂端就在目前頁內時保留頁內位置，否則（目前頁是下面那頁）改成對齊目前頁頂端。
      var top = readAnchor();
      state.anchor = top.page === state.page ? top : { page: state.page, offset: 0 };
      state.zoom = zoom;
      layout();
      applyAnchor();
      scroller.scrollLeft = Math.max(0, centerX * scroller.scrollWidth - scroller.clientWidth / 2);
      updateToolbar();
    }

    function stepZoom(direction) {
      var current = currentZoom();
      if (current === null) {
        return;
      }
      var steps = current / PDF_ZOOM_STEP;
      var next = direction > 0 ? (Math.floor(steps + PDF_ZOOM_EPSILON) + 1) * PDF_ZOOM_STEP : (Math.ceil(steps - PDF_ZOOM_EPSILON) - 1) * PDF_ZOOM_STEP;
      setZoom(clamp(next, PDF_ZOOM_MIN, PDF_ZOOM_MAX));
    }

    function goToPage(num) {
      if (num < 1 || num > numPages) {
        return;
      }
      state.page = num;
      state.anchor = { page: num, offset: 0 };
      if (laidOut) {
        applyAnchor();
      }
      updateToolbar();
    }

    function setDisabled(button, disabled) {
      var value = disabled ? "true" : "false";
      if (button.getAttribute("aria-disabled") !== value) {
        button.setAttribute("aria-disabled", value);
      }
    }

    function updateToolbar() {
      var text = state.page + " / " + numPages;
      if (pageStatus.textContent !== text) {
        pageStatus.textContent = text;
      }
      setDisabled(prevButton, state.page <= 1);
      setDisabled(nextButton, state.page >= numPages);
      var zoom = currentZoom();
      var zoomText = zoom === null ? "—" : Math.round(zoom * 100) + "%";
      if (zoomLevel.textContent !== zoomText) {
        zoomLevel.textContent = zoomText;
      }
      setDisabled(zoomOutButton, zoom === null || zoom <= PDF_ZOOM_MIN + PDF_ZOOM_EPSILON);
      setDisabled(zoomInButton, zoom === null || zoom >= PDF_ZOOM_MAX - PDF_ZOOM_EPSILON);
      var pressed = state.zoom === "fit" ? "true" : "false";
      if (fitButton.getAttribute("aria-pressed") !== pressed) {
        fitButton.setAttribute("aria-pressed", pressed);
      }
    }

    // --- 畫頁面 ---

    function cancelRender(p) {
      if (p.task !== null) {
        var task = p.task;
        p.task = null;
        p.taskKey = null;
        task.cancel();
      }
    }

    function dropCanvas(p) {
      cancelRender(p);
      if (p.key !== null) {
        var blank = blankCanvas();
        p.box.replaceChild(blank, p.canvas);
        p.canvas.width = 0; // 立刻還掉點陣圖記憶體，不等 GC
        p.canvas.height = 0;
        p.canvas = blank;
        p.key = null;
      }
    }

    // 已畫好的頁面超過上限時，丟掉不在可視範圍附近、離目前頁最遠的那幾張。
    function trimRendered() {
      var rendered = pages.filter(function (p) {
        return p.key !== null;
      });
      while (rendered.length > PDF_RENDERED_PAGES_MAX) {
        var victim = null;
        rendered.forEach(function (p) {
          if (!p.near && (victim === null || Math.abs(p.num - state.page) > Math.abs(victim.num - state.page))) {
            victim = p;
          }
        });
        if (victim === null) {
          return;
        }
        dropCanvas(victim);
        rendered.splice(rendered.indexOf(victim), 1);
      }
    }

    function renderPage(p) {
      if (released || !laidOut) {
        return;
      }
      var ratio = Math.min(window.devicePixelRatio || 1, Math.sqrt(PDF_MAX_CANVAS_PIXELS / (p.cssW * p.cssH)));
      var key = p.cssW + "x" + p.cssH + "@" + ratio.toFixed(3);
      if (p.key === key || p.taskKey === key) {
        return;
      }
      cancelRender(p);
      var viewport = p.proxy.getViewport({ scale: zoomUsed * cssUnits * ratio });
      var canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.floor(viewport.width));
      canvas.height = Math.max(1, Math.floor(viewport.height));
      var task = p.proxy.render({ canvas: canvas, viewport: viewport });
      p.task = task;
      p.taskKey = key;
      task.promise.then(
        function () {
          if (p.task !== task || released) {
            return;
          }
          p.task = null;
          p.taskKey = null;
          p.box.replaceChild(canvas, p.canvas);
          p.canvas.width = 0;
          p.canvas.height = 0;
          p.canvas = canvas;
          p.key = key;
          trimRendered();
        },
        function (error) {
          if (p.task === task) {
            p.task = null;
            p.taskKey = null;
          }
          canvas.width = 0;
          canvas.height = 0;
          if (!released && !(error instanceof lib.RenderingCancelledException)) {
            console.warn("PDF 第 " + p.num + " 頁畫不出來", error);
          }
        }
      );
    }

    var observer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          var num = Number(entry.target.getAttribute("data-page"));
          var p = pages[num - 1];
          if (p === undefined) {
            return;
          }
          p.near = entry.isIntersecting;
          if (p.near) {
            renderPage(p);
          } else {
            cancelRender(p); // 離開（含分頁被隱藏）：還沒畫完的不必畫完
          }
        });
      },
      { root: scroller, rootMargin: PDF_NEAR_MARGIN }
    );

    // 捲動區尺寸改變：第一次排版、符合寬度下改寬重排；從隱藏變回可見（尺寸由 0 變回來）時捲回錨點。
    var resizer = new ResizeObserver(function () {
      if (released || scroller.clientWidth === 0) {
        return;
      }
      if (!laidOut || (state.zoom === "fit" && scroller.clientWidth !== lastWidth)) {
        layout();
      }
      applyAnchor();
      updateToolbar();
    });

    scroller.addEventListener("scroll", function () {
      if (rafPending) {
        return;
      }
      rafPending = true;
      requestAnimationFrame(function () {
        rafPending = false;
        if (released || !laidOut || scroller.clientWidth === 0) {
          return;
        }
        state.anchor = readAnchor();
        if (programmaticTop !== null && scroller.scrollTop === programmaticTop) {
          return; // 自己捲的（翻頁、縮放、捲回錨點）：目前頁已由該動作決定
        }
        programmaticTop = null;
        state.page = mostVisiblePage();
        updateToolbar();
      });
    });

    updateToolbar();

    return {
      viewer: "pdf",
      view: view,
      start: function () {
        resizer.observe(scroller);
      },
      state: function () {
        return {
          viewer: "pdf",
          page: state.page,
          numPages: numPages,
          zoom: state.zoom,
          anchor: { page: state.anchor.page, offset: state.anchor.offset },
        };
      },
      release: function () {
        if (released) {
          return;
        }
        released = true;
        observer.disconnect();
        resizer.disconnect();
        pages.forEach(function (p) {
          cancelRender(p);
          p.canvas.width = 0;
          p.canvas.height = 0;
        });
        quietly(loadingTask.destroy());
      },
    };
  }

  function pdfViewer(ctx) {
    var host = ctx.host;
    var size = ctx.meta ? ctx.meta.size : undefined;
    var load = {
      cancelled: false,
      task: null,
      destroy: function () {
        this.cancelled = true;
        if (this.task !== null) {
          quietly(this.task.destroy());
        }
      },
    };
    var earlier = pendingLoads.get(host);
    if (earlier !== undefined) {
      earlier.destroy(); // 同一個 host 上較晚的載入取代較早的
    }
    pendingLoads.set(host, load);
    var stale = function () {
      return load.cancelled || !ctx.isCurrent();
    };
    var settle = function () {
      if (pendingLoads.get(host) === load) {
        pendingLoads.delete(host);
      }
      if (stale() && load.task !== null) {
        quietly(load.task.destroy());
      }
    };
    var unparsable = function (error) {
      console.warn(PDF_UNPARSABLE_TEXT + "：" + ctx.file.path, error);
      if (load.task !== null) {
        quietly(load.task.destroy());
      }
      mount(ctx, noteView("pdf", PDF_UNPARSABLE_TEXT, size));
      return { ok: true };
    };

    var library = loadPdfjs().then(
      function (lib) {
        return lib;
      },
      function (error) {
        console.warn("pdf.js 載入失敗", error);
        return null;
      }
    );
    return Promise.all([getBody(ctx.rawUrl, "arrayBuffer", PDF_REQUEST_TIMEOUT_MS), library]).then(function (results) {
      var response = results[0];
      var lib = results[1];
      if (stale()) {
        settle();
        return undefined;
      }
      if (!response.ok) {
        settle();
        return { ok: false, code: response.code };
      }
      if (lib === null) {
        settle();
        return { ok: false, code: "unknown" };
      }
      var task;
      try {
        task = lib.getDocument(pdfDocumentParams(new Uint8Array(response.body)));
      } catch (error) {
        settle();
        return unparsable(error);
      }
      load.task = task;
      return task.promise
        .then(function (doc) {
          if (stale()) {
            return null;
          }
          var gets = [];
          for (var i = 1; i <= doc.numPages; i += 1) {
            gets.push(doc.getPage(i));
          }
          return Promise.all(gets);
        })
        .then(
          function (proxies) {
            if (proxies === null || stale()) {
              settle();
              return undefined;
            }
            settle();
            var previous = hostSessions.get(host);
            var carry = previous !== undefined && previous.viewer === "pdf" ? previous.state() : null;
            var session = createPdfSession(lib, task, proxies, carry);
            mount(ctx, session.view); // 換掉（並釋放）舊內容
            hostSessions.set(host, session);
            session.start();
            return { ok: true };
          },
          function (error) {
            if (stale()) {
              settle();
              return undefined;
            }
            settle();
            return unparsable(error);
          }
        );
    });
  }

  // 釋放 host 上的檢視器資源（PDF 的文件、worker、render task；還在讀的 PDF 載入）。files.js 關閉分頁時呼叫。
  function releaseHost(host) {
    var pending = pendingLoads.get(host);
    if (pending !== undefined) {
      pendingLoads.delete(host);
      pending.destroy();
    }
    releaseSession(host);
  }

  function hostState(host) {
    var session = hostSessions.get(host);
    return session !== undefined ? session.state() : null;
  }

  window.cockpitViewers = {
    markdown: markdownViewer,
    text: textViewer,
    html: htmlViewer,
    pdf: pdfViewer,
    unsupported: unsupportedViewer,
  };

  window.cockpitMarkdownAnchor = { reveal: revealAnchor };

  window.cockpitViewerHost = { release: releaseHost, state: hostState };
})();
