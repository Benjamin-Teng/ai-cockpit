// git.js：git-review「變更」面板、diff 分頁、Git Graph 分頁、某版本分頁（design D9）。
//
// git-review task 3.3 起是空殼檔案；task 4.1 補上分頁 kind 骨架（"diff"／"graph"／"rev" 三個佔位
// kind，讓 files.js 的分頁框架認得它們）。task 4.2 做兩件事：
//   1. 左欄「變更」面板（`#changes-panel`；spec git-review「左欄變更分頁」）：本檔自己建立內容 DOM、
//      自己查詢、自己輪詢，files.js 只切換 `#changes-panel` 的 hidden 並在進出這個分頁時呼叫
//      `changesTabEntered()`／`changesTabLeft()`（見檔尾「對外」）。
//   2. `diff`／`graph` 兩個 kind 的 `identity()`／`create()` 改成 design D9 真正的分頁身分與標題；
//      內容仍是「尚未實作」的占位文字（task 4.1 遺留的骨架）。`rev` kind 維持 task 4.1 的骨架不動。
//
// task 4.3 補上 diff 分頁的真正內容（spec git-review「diff 分頁」；design D7）：左右並排格線、
// 工具列四個動作（開啟檔案／在 VS Code 開啟／看左側版本／看右側版本）、輪詢與 `version` 比對、
// 捲動保留、過期標示、分頁還原（`serialize`／`deserialize`）。順便把 `rev` kind 的 `identity()`／
// `create()` 升級成 design D9 真正的分頁身分與標題（內容仍是占位文字，等 task 4.5）——因為 diff
// 工具列的「看左側版本」「看右側版本」現在要能以正確身分開出 `rev` 分頁。Git Graph 的內容仍是
// task 4.4 的事。
//
// 對外：`window.cockpitGit = { kinds, paneSelected(runtime, paneId), paneCleared(),
// setKnownPanes(panes), changesTabEntered(), changesTabLeft() }`。後四個沿用
// `window.liveOutput`／`window.cockpitFiles` 的既有掛勾慣例：output.js 的 select()／clear() 與
// render.js 的每次重畫都會呼叫到（見 output.js「選取改變時通知 files.js」、render.js
// `window.cockpitFiles.setKnownPanes` 呼叫處），跟 files.js 的檔案樹是同一份 pane 選取來源、同一份
// cwd 投影，只是各自維護自己的「目前查的是哪個根目錄」狀態，互不干涉（各自的請求只在自己的分頁可見
// 時才會發出，design D9／spec「左欄變更分頁」：「變更」分頁不是目前分頁時不查）。
//
// --- 變更面板（spec git-review「左欄變更分頁」；design D9；memory full-repaint-discards-state-
// held-only-in-dom：狀態存在模組變數，DOM 只是呈現）---
//
// 根目錄來源與 files.js 的檔案樹完全對稱（同一個選取、同一份 cwd 投影），但狀態各自獨立維護
// （不共用 files.js 的私有變數）：
//   - `selectedPane`／`paneCwds`／`view`／`viewGen`：同 files.js 的同名概念——`view` 記著「目前選定的
//     pane 查到的根目錄」（`status: "loading"|"ok"|"error"`），`viewGen` 讓換選取後才到的舊查詢被
//     丟棄。查詢只在「變更」分頁目前可見時才發出（`isChangesActive()`），這是本面板與檔案樹之間唯一
//     的差別（檔案樹是「檔案」分頁可見時才查）。
//   - 根目錄查到、且 `root.is_git` 為真時才查 git 狀態（`/api/git/<rt>/<root_id>/status`）：
//     `gitStatus = { status: "idle"|"ok"|"error", code, body }`，換根目錄（`switchShownRoot()`）時
//     重置——這個面板沒有 files.js 檔案樹那種「跨根目錄保留狀態」的需求（spec 沒有要求），簡化為
//     「換根目錄＝清空重來」。
//   - 輪詢（`poll = { gen, timer, controller }`）：只在「變更」分頁可見、且已知是 git repo 時才有
//     輪詢鏈；`stopPolling()` 讓世代加一、清掉排定的下一次、abort 進行中的那一筆，任何「離開變更
//     分頁」「選取改變」「根目錄改變」都呼叫它，跟 files.js 檔案分頁的自動更新是同一套時序模型
//     （切換立即查、之後每次讀取結束 2 秒後再查一次）。
//   - 清單以「鍵」（`group + 路徑 + 原路徑`）逐列比對更新（`renderChangesList()`，作法同 files.js 的
//     `renderTree()`）：同一份資料重讀不換掉任何節點，焦點所在的列（原生 `<button>`，Enter／Space
//     直接觸發 `click`，不必自己處理鍵盤）只要鍵不變就是同一個 DOM 節點，瀏覽器的焦點自然不會被
//     打斷（不需要另外記錄／還原「聚焦在哪一列」）。捲動位置另外用一個模組變數
//     （`changesScrollTop`，經 `scroll` 事件持續記錄）在分頁從 hidden 變回可見時寫回——被 hidden 的
//     捲動容器 scrollTop 不保證保留（同 files.js 檔案樹 `restoreTreeScroll()` 的理由）。換根目錄時
//     捲動位置歸零（新的清單，跟「展開狀態跨根目錄保留」是不同需求，這裡沒有這個要求）。
//   - 讀取失敗時保留上一次成功的 `gitStatus.body`（不清空），面板加上過期標示（沿用
//     `.file-stale-label` 與 `.files-status[data-tone="warn"]` 這兩個既有選擇器，過期色條用行內
//     `boxShadow` 而不是新增一條只有這裡用得到的 CSS 選擇器——同 git.js task 4.1 骨架「拿掉
//     `.git-placeholder`」的教訓：新選擇器要有辦法在某個畫面狀態下被摸到，行內樣式沒有這個問題）。
//   - 狀態字母的顏色（design 控制端裁決：M→--warn、A／?→--ok、D→--bad、R／C→--accent、T→--warn、
//     U→--bad）同樣用行內 `style.color = "var(--token)"` 而不是 `[data-status="…"]` 系列選擇器：
//     working-tree 狀態實際只會出現 M／D／?／U 四種（`A`／`R`／`C`／`T` 要在 commit 之間才會出現，
//     status 端點的 fixture 沒有這些狀態），開 5 條屬性選擇器裡 4 條在任何畫面狀態下都摸不到元素，
//     必然是 CL1 死規則；行內樣式一樣「只引用 token」（design 對顏色的唯一要求），不受這個限制。
//
// --- diff／graph 分頁身分（design D9）---
//
// `diff` 為 runtime＋root_id＋from＋to＋路徑＋原路徑；`graph` 為 runtime＋root_id（每個 repo 一個）。
// 本 task 只開分頁、不做內容（task 4.3／4.4），create() 沿用 4.1 骨架的「尚未實作」占位文字，但
// label／title 已經是最終形狀，4.3／4.4 直接接手內容就好，不必回頭修分頁列。

(function () {
  "use strict";

  var reviewRoot = document.getElementById("review");
  var reviewTablist = reviewRoot ? reviewRoot.querySelector('[role="tablist"]') : null;
  var tabSeq = 0;

  // --- 小工具（同 files.js／output.js／actions.js 的既有慣例：各檔案自己維護一份，見 files.js
  // 檔頭「重複的小工具」——這幾行太小，抽成共用模組的成本比重複本身還高）---

  var ICON_BASE = "/vendor/material-icons/icons/";
  var DEFAULT_FILE_ICON = "file.svg"; // 同 files.js：對照表的預設檔案 icon（material-icons.json 的 "file"）
  var REQUEST_TIMEOUT_MS = 10000;
  var POLL_INTERVAL_MS = 2000; // spec「左欄變更分頁」／「diff 分頁」：每次讀取結束 2 秒後再讀取一次

  function seg(value) {
    return encodeURIComponent(value);
  }

  function setAttr(el, name, value) {
    if (value === null) {
      if (el.hasAttribute(name)) {
        el.removeAttribute(name);
      }
    } else if (el.getAttribute(name) !== value) {
      el.setAttribute(name, value);
    }
  }

  function setHidden(el, hidden) {
    if (el.hidden !== hidden) {
      el.hidden = hidden;
    }
  }

  function baseName(path) {
    return path.slice(path.lastIndexOf("/") + 1);
  }

  // 一次 GET：resolve 成 `{ ok: true, body }` 或 `{ ok: false, code }`（不 reject）。同 files.js
  // 的 `getJson()`：本體不是 JSON 或沒有 `code` 時為 "unknown"，連線失敗為 "network"，前端逾時為
  // "timeout"。
  function getJson(url, signal) {
    var controller = new AbortController();
    var timedOut = false;
    var timer = setTimeout(function () {
      timedOut = true;
      controller.abort();
    }, REQUEST_TIMEOUT_MS);
    if (signal) {
      if (signal.aborted) {
        controller.abort();
      } else {
        signal.addEventListener(
          "abort",
          function () {
            controller.abort();
          },
          { once: true }
        );
      }
    }
    return fetch(url, { signal: controller.signal, cache: "no-store" })
      .then(function (response) {
        return response.json().then(
          function (body) {
            if (response.ok) {
              return { ok: true, body: body };
            }
            return { ok: false, code: body && typeof body.code === "string" ? body.code : "unknown" };
          },
          function () {
            return { ok: false, code: timedOut ? "timeout" : "unknown" };
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

  function rootUrl(runtime, paneId) {
    return "/api/runtimes/" + seg(runtime) + "/panes/" + seg(paneId) + "/root";
  }

  function statusUrl(runtime, rootId) {
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/status";
  }

  // diff 端點（spec git-review「單檔 diff 端點」）：`from`／`to` 是版本字面值（hash／INDEX／WORKTREE／
  // EMPTY），`path`／`old_path` 逐段 encode 再以 `/` 接回（同 files.js 的 fileUrl()：伺服器逐段
  // percent-decode，不能先整段 encode 再整段 decode，否則 `%2F` 會被誤判成路徑分隔字元）。
  function diffUrl(runtime, rootId, from, to, path, oldPath) {
    var q = "from=" + seg(from) + "&to=" + seg(to) + "&path=" + path.split("/").map(seg).join("/");
    if (oldPath) {
      q += "&old_path=" + oldPath.split("/").map(seg).join("/");
    }
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/diff?" + q;
  }

  // file-review 的中繼資料端點（同 files.js 的 fileUrl("meta", …)）：diff 分頁的「開啟檔案」「在
  // VS Code 開啟」兩個動作要知道目前工作區有沒有這個檔案、`vscode_uri`，用的是同一個端點（不是
  // git 端點）——git.js 不依賴 files.js 的私有函式，這裡重複一份（同檔頭「重複的小工具」慣例）。
  function filesMetaUrl(runtime, rootId, path) {
    return "/api/files/" + seg(runtime) + "/" + seg(rootId) + "/meta/" + path.split("/").map(seg).join("/");
  }

  // 根目錄查詢失敗的文案（同 files.js 的 ERROR_TEXT：依錯誤本體的 `code` 查表，不顯示 HTTP 狀態碼或
  // API 路徑）。
  var ROOT_ERROR_TEXT = {
    forbidden_source: "請求來源不被接受，請從本機的 Cockpit 頁面開啟",
    method_not_allowed: "這個操作不被接受",
    bad_request: "名稱含有無法處理的字元，無法讀取",
    runtime_unknown: "設定中沒有這個 runtime",
    pane_unknown: "這個 pane 已不在目前的畫面中",
    no_root: "這個 pane 沒有可瀏覽的資料夾（沒有回報工作目錄，或該目錄不存在）",
    root_unavailable: "這個根目錄目前無法瀏覽（不在允許的根目錄中）",
    network: "無法連線到 Cockpit 服務",
    timeout: "讀取逾時（超過 10 秒沒有回應）",
  };

  function errorText(code) {
    return Object.prototype.hasOwnProperty.call(ROOT_ERROR_TEXT, code) ? ROOT_ERROR_TEXT[code] : "讀取失敗（無法辨識的回應）";
  }

  // git 狀態端點失敗的文案（spec git-review「git 端點的共同規則」的代碼；文字對照
  // `cockpit::git::GitApiError::reason()`，但這裡是獨立維護的前端文案，不是同一份字串）。
  var GIT_ERROR_TEXT = {
    not_git: "這個根目錄不是 git repo",
    bad_request: "請求格式不正確",
    git_unavailable: "找不到可用的 git",
    git_untrusted: "git 拒絕讀取這個 repo（擁有者不符）",
    git_timeout: "執行逾時",
    git_failed: "執行 git 時發生錯誤",
    network: "無法連線到 Cockpit 服務",
    timeout: "讀取逾時（超過 10 秒沒有回應）",
    // git-review task 4.3：diff 端點另外會用到的錯誤代碼（spec「git 端點的共同規則」）。
    rev_unknown: "找不到這個版本",
    ref_unknown: "找不到這個分支",
    not_found_in_rev: "檔案在這個版本不存在",
    no_merge_base: "兩者沒有共同祖先",
    too_large: "差異過大，請在 VS Code 查看",
    // 目視驗收缺陷 V1、Ruling R11：對未合併（衝突中）的檔案請求 INDEX→WORKTREE 時的錯誤代碼。
    unmerged_path: "這個檔案還在合併衝突中，暫存區沒有單一版本可比較",
  };

  function gitErrorText(code) {
    return Object.prototype.hasOwnProperty.call(GIT_ERROR_TEXT, code) ? GIT_ERROR_TEXT[code] : "讀取失敗（無法辨識的回應）";
  }

  // ---------------------------------------------------------------------------
  // 分頁 kind：diff（task 4.3 起有真正內容）／graph（仍是占位）／rev（身分與標題已是 design D9
  // 最終形狀，內容仍是占位——task 4.5 才補）
  // ---------------------------------------------------------------------------

  // 建立一個下半部分頁的外殼（分頁本身＋關閉鈕＋空 tabpanel），自己 appendChild 進分頁列與 `#review`
  // （同 files.js FILE_KIND 的慣例）。`iconName` 可省略（Graph 分頁沒有 icon 需求；diff／rev 分頁
  // 有）；提供時外觀與 file 分頁的 `.review-tab-icon` 完全相同（重用既有選擇器）。回傳的
  // `els.panel` 是空的，內容（占位文字，或 diff 分頁自己的工具列／格線）由呼叫端自己填。
  function buildTabShell(kind, label, iconName) {
    if (reviewTablist === null || reviewRoot === null) {
      return null;
    }
    tabSeq += 1;
    var n = tabSeq;
    var idBase = "review-tab-" + kind + n;

    var wrap = document.createElement("div");
    wrap.className = "review-tab";
    wrap.setAttribute("role", "presentation");

    var tab = document.createElement("button");
    tab.type = "button";
    tab.className = "review-tab-main";
    tab.id = idBase;
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-selected", "false");
    tab.setAttribute("aria-controls", "review-panel-" + kind + n);
    tab.tabIndex = -1;

    var iconEl = null;
    if (typeof iconName === "string" && iconName !== "") {
      iconEl = document.createElement("img");
      iconEl.className = "review-tab-icon";
      iconEl.alt = "";
      iconEl.width = 16;
      iconEl.height = 16;
      iconEl.draggable = false;
      iconEl.src = ICON_BASE + seg(iconName);
      tab.appendChild(iconEl);
    }
    var tabLabel = document.createElement("span");
    tabLabel.className = "review-tab-label";
    tabLabel.textContent = label;
    tab.appendChild(tabLabel);

    var close = document.createElement("button");
    close.type = "button";
    close.className = "review-tab-close";
    close.tabIndex = -1;
    close.setAttribute("aria-label", "關閉 " + label);
    close.title = "關閉";
    close.textContent = "×";

    wrap.appendChild(tab);
    wrap.appendChild(close);
    reviewTablist.appendChild(wrap);

    var panel = document.createElement("div");
    panel.className = "review-panel";
    panel.id = "review-panel-" + kind + n;
    panel.setAttribute("role", "tabpanel");
    panel.setAttribute("aria-labelledby", tab.id);
    panel.hidden = true;
    reviewRoot.appendChild(panel);

    return { els: { wrap: wrap, tab: tab, close: close, panel: panel, icon: iconEl, label: tabLabel } };
  }

  // 「尚未實作」占位內容（graph／rev 目前都是這樣）：沿用既有的 `.file-status` 選擇器而不是新增一條
  // 只有這裡用得到的規則（task 4.1「自我審查」CL1 死規則的教訓）。
  function buildPlaceholderTabDom(kind, label, bodyText, iconName) {
    var shell = buildTabShell(kind, label, iconName);
    if (shell === null) {
      return null;
    }
    var body = document.createElement("p");
    body.className = "file-status";
    body.style.padding = "16px";
    body.textContent = bodyText;
    shell.els.panel.appendChild(body);
    return shell;
  }

  // 短 hash：跟 Git Graph／commit 詳情（task 4.4／4.5）預定使用的長度一致（git 常見的 7 碼縮寫）。
  function shortHash(oid) {
    return typeof oid === "string" && oid.length > 7 ? oid.slice(0, 7) : oid;
  }

  // 比較對象的顯示文字（spec git-review「diff 分頁」：「工作區」「已暫存」、commit 短 hash，或
  // 「短 hash ↔ 短 hash」——本 task 只會遇到前兩種，commit 相關的顯示留給 task 4.5 的比較功能擴充）。
  function sideLabel(version) {
    if (version === "WORKTREE") {
      return "工作區";
    }
    if (version === "INDEX") {
      return "已暫存";
    }
    if (version === "EMPTY") {
      return "（空）";
    }
    return shortHash(version);
  }

  function diffIdentity(fields) {
    return fields.runtime + "\n" + fields.rootId + "\n" + fields.from + "\n" + fields.to + "\n" + fields.path + "\n" + (fields.oldPath || "");
  }

  // 任一側為工作區或暫存區時要輪詢（spec「diff 分頁」）；兩側都是 commit（或 EMPTY，效果同 commit：
  // 內容不會變）時只讀一次，不重複讀取。
  function diffNeedsPolling(tab) {
    return tab.from === "WORKTREE" || tab.from === "INDEX" || tab.to === "WORKTREE" || tab.to === "INDEX";
  }

  // 特殊結果的文案（design D7；優先序：二進位 → 只有權限改變 → 子模組 → 兩側內容相同）；回 null 代表
  // 應該顯示正常的左右並排格線。
  function diffSpecialText(body) {
    if (body.binary) {
      return "二進位檔，不顯示差異";
    }
    if (body.mode_only) {
      return "只有權限改變";
    }
    if (body.submodule) {
      return "子模組，不顯示差異";
    }
    if (body.rows.length === 0) {
      return "兩側內容相同";
    }
    return null;
  }

  // gap 列橫跨四欄；其餘列依 kind 決定左右兩側各自的「角色」（context／del／add／blank），只有一側
  // 有內容的列（delete／add）另一側畫成較暗的空白格（design D7「特殊結果」；spec「diff 分頁」）。
  function diffRowExtraClass(sideKind) {
    if (sideKind === "del") {
      return " diff-row-del";
    }
    if (sideKind === "add") {
      return " diff-row-add";
    }
    if (sideKind === "blank") {
      return " diff-row-blank";
    }
    return "";
  }

  function appendDiffCellPair(frag, sideKind, line) {
    var extra = diffRowExtraClass(sideKind);
    var num = document.createElement("div");
    num.className = "diff-num" + extra;
    num.textContent = line ? String(line.line) : "";
    var text = document.createElement("div");
    text.className = "diff-text" + extra;
    text.textContent = line ? line.text : "";
    frag.appendChild(num);
    frag.appendChild(text);
  }

  function appendDiffRow(frag, row) {
    if (row.kind === "gap") {
      var gapEl = document.createElement("div");
      gapEl.className = "diff-gap";
      gapEl.textContent = "省略 " + row.lines + " 行";
      frag.appendChild(gapEl);
      return;
    }
    var leftKind = "context";
    var rightKind = "context";
    if (row.kind === "delete") {
      leftKind = "del";
      rightKind = "blank";
    } else if (row.kind === "add") {
      leftKind = "blank";
      rightKind = "add";
    } else if (row.kind === "change") {
      leftKind = "del";
      rightKind = "add";
    }
    appendDiffCellPair(frag, leftKind, row.left || null);
    appendDiffCellPair(frag, rightKind, row.right || null);
  }

  // 重建格線內容（`version` 改變才呼叫；DocumentFragment 一次插入，不逐列觸發排版）。維持捲動位置
  // （brief「大量列的效能」；memory full-repaint-discards-state-held-only-in-dom：捲動只在畫面上，
  // 內容重建前後要自己讀寫）。特殊結果／兩側相同時清空格線（狀態文字由 renderDiffPanel() 顯示）。
  function rebuildDiffGrid(tab) {
    var host = tab.els.host;
    var showGrid = diffSpecialText(tab.diffBody) === null;
    if (!showGrid) {
      host.replaceChildren();
      tab.els.grid = null;
      return;
    }
    var prevTop = host.scrollTop;
    var grid = document.createElement("div");
    grid.className = "diff-grid";
    grid.setAttribute("data-viewer", "diff");
    var frag = document.createDocumentFragment();
    tab.diffBody.rows.forEach(function (row) {
      appendDiffRow(frag, row);
    });
    grid.appendChild(frag);
    host.replaceChildren(grid);
    tab.els.grid = grid;
    // 過期時格線文字色的行內覆寫（見下方 renderDiffPanel()）在重建後要重新套用，否則新節點會是
    // 預設（未過期）的顏色。
    grid.style.color = tab.diffStatus === "error" && tab.diffBody !== null ? "var(--text-dim)" : "";
    host.scrollTop = prevTop; // 瀏覽器自動夾在 [0, scrollHeight-clientHeight]，超出時取最大值
  }

  // 工具列、狀態列、過期標示與格線顯示與否（不重建格線本身；只有 rebuildDiffGrid() 改變格線內容）。
  function renderDiffPanel(tab) {
    var els = tab.els;
    var pathText = tab.oldPath ? tab.oldPath + " → " + tab.path : tab.path;
    if (els.pathEl.textContent !== pathText) {
      els.pathEl.textContent = pathText;
    }
    setAttr(els.pathEl, "title", pathText);
    var versionsText = sideLabel(tab.from) + " → " + sideLabel(tab.to);
    if (els.versionsEl.textContent !== versionsText) {
      els.versionsEl.textContent = versionsText;
    }

    // 工具列四個動作（spec「diff 分頁」）。
    var wf = tab.workFile;
    var canOpen = wf !== null && wf.exists === true;
    els.openFileBtn.disabled = !canOpen;
    setHidden(els.vscodeLink, !(canOpen && wf.vscodeUri !== null));
    setAttr(els.vscodeLink, "href", canOpen && wf.vscodeUri !== null ? wf.vscodeUri : null);
    setHidden(els.leftVersionBtn, tab.from === "EMPTY");
    setHidden(els.rightVersionBtn, tab.to === "EMPTY");
    // 目視驗收缺陷 V2、Ruling R12：指向工作區的那一側「看…版本」等同「開啟檔案」（見
    // openDiffLeftVersion／openDiffRightVersion：那一側是 WORKTREE 時開的就是目前工作區版本的
    // 檔案分頁），工作區沒有這個檔案時同樣要停用，不能只停用「開啟檔案」。非工作區那一側（commit
    // 或暫存區）一律可用，不受 `canOpen`（工作區有沒有這個檔案）影響。
    els.leftVersionBtn.disabled = tab.from === "WORKTREE" && !canOpen;
    els.rightVersionBtn.disabled = tab.to === "WORKTREE" && !canOpen;

    // 過期標示（spec：讀取失敗時保留上一次的內容並以過期標示呈現；沒有內容可保留時只顯示原因，同
    // file-review 檔案分頁的既有做法）。格線文字色用行內樣式覆寫，不新增只有「diff 讀取失敗」這個
    // 少數狀態才碰得到的 CSS 選擇器（同 git.js「變更面板」行內樣式的既有裁決，見檔頭）。
    var stale = tab.diffStatus === "error" && tab.diffBody !== null;
    if (els.panel.classList.contains("is-stale") !== stale) {
      els.panel.classList.toggle("is-stale", stale);
    }
    setHidden(els.staleLabel, !stale);
    if (els.grid !== null) {
      els.grid.style.color = stale ? "var(--text-dim)" : "";
    }

    var text = null;
    var tone = "dim";
    if (tab.diffStatus === "error") {
      text = gitErrorText(tab.diffCode);
      tone = "warn";
    } else if (tab.diffBody === null) {
      text = "正在讀取差異…";
    } else {
      text = diffSpecialText(tab.diffBody);
    }
    setHidden(els.status, text === null);
    if (text !== null && els.status.textContent !== text) {
      els.status.textContent = text;
    }
    setAttr(els.status, "data-tone", tone);

    var showGrid = tab.diffBody !== null && diffSpecialText(tab.diffBody) === null;
    setHidden(els.host, !showGrid);
  }

  function applyDiffResult(tab, result) {
    if (tab.closed) {
      return;
    }
    if (result.ok && result.body && typeof result.body.version === "string" && Array.isArray(result.body.rows)) {
      tab.diffStatus = "ok";
      tab.diffCode = null;
      if (tab.diffVersion !== result.body.version) {
        tab.diffVersion = result.body.version;
        tab.diffBody = result.body;
        rebuildDiffGrid(tab);
      }
    } else {
      tab.diffStatus = "error";
      tab.diffCode = result.ok ? "unknown" : result.code;
    }
    renderDiffPanel(tab);
  }

  // 工作區是否有這個檔案、`vscode_uri`（「開啟檔案」「在 VS Code 開啟」兩個動作要用；spec「diff
  // 分頁」「在 VS Code 開啟」）。順便更新分頁 icon（file-review 中繼資料端點的 icon 依副檔名查，比
  // 開分頁當下拿到的 `fields.icon`——若有——更準；查不到工作區檔案時保留原本的 icon）。
  function refreshDiffWorkFile(tab) {
    tab.workGen += 1;
    var gen = tab.workGen;
    getJson(filesMetaUrl(tab.runtime, tab.rootId, tab.path)).then(function (result) {
      if (tab.closed || gen !== tab.workGen) {
        return;
      }
      if (result.ok && result.body && typeof result.body === "object") {
        var vscodeUri = typeof result.body.vscode_uri === "string" && result.body.vscode_uri !== "" ? result.body.vscode_uri : null;
        tab.workFile = { exists: true, vscodeUri: vscodeUri };
        if (typeof result.body.icon === "string" && result.body.icon !== "" && tab.els.icon !== null) {
          setAttr(tab.els.icon, "src", ICON_BASE + seg(result.body.icon));
        }
      } else {
        tab.workFile = { exists: false, vscodeUri: null };
      }
      renderDiffPanel(tab);
    });
  }

  // 輪詢（同 files.js FILE_KIND 的單例輪詢：同一時間只有一個分頁是目前分頁，輪詢只服務目前分頁，
  // 所以一個模組變數夠用）。一次性讀取（兩側都是 commit）用各自分頁自己的 `diffGen`／
  // `diffController`，不用這個共用變數——它們不必等被切走才停，dispose() 直接 abort 即可。
  var diffPoll = { gen: 0, timer: null, controller: null };

  function stopDiffPolling() {
    diffPoll.gen += 1;
    if (diffPoll.timer !== null) {
      clearTimeout(diffPoll.timer);
      diffPoll.timer = null;
    }
    if (diffPoll.controller !== null) {
      diffPoll.controller.abort();
      diffPoll.controller = null;
    }
  }

  function startDiffPolling(tab) {
    stopDiffPolling();
    fetchDiffPoll(tab, diffPoll.gen);
  }

  function fetchDiffPoll(tab, gen) {
    if (gen !== diffPoll.gen || tab.closed) {
      return;
    }
    var controller = new AbortController();
    diffPoll.controller = controller;
    refreshDiffWorkFile(tab);
    getJson(diffUrl(tab.runtime, tab.rootId, tab.from, tab.to, tab.path, tab.oldPath), controller.signal).then(function (result) {
      if (gen !== diffPoll.gen) {
        return; // 切換／關閉之後才回來（含被 stopDiffPolling() 中止的那一筆）：丟棄
      }
      diffPoll.controller = null;
      applyDiffResult(tab, result);
      diffPoll.timer = setTimeout(function () {
        diffPoll.timer = null;
        fetchDiffPoll(tab, gen);
      }, POLL_INTERVAL_MS);
    });
  }

  // 一次性讀取（兩側都是 commit，或 EMPTY→commit）：分頁建立時讀一次，之後不再重複讀取（spec「兩側
  // 都是 commit 時不重複讀取」）。
  function fetchDiffOnce(tab) {
    var controller = new AbortController();
    tab.diffController = controller;
    var gen = tab.diffGen;
    getJson(diffUrl(tab.runtime, tab.rootId, tab.from, tab.to, tab.path, tab.oldPath), controller.signal).then(function (result) {
      if (tab.closed || gen !== tab.diffGen) {
        return;
      }
      tab.diffController = null;
      applyDiffResult(tab, result);
    });
  }

  function captureDiffScroll(tab) {
    tab.scroll.top = tab.els.host.scrollTop;
  }

  function restoreDiffScroll(tab) {
    tab.els.host.scrollTop = tab.scroll.top;
  }

  // 「開啟檔案」「看左側版本」為 WORKTREE 時「看右側版本」：開啟目前工作區版本的檔案分頁。
  function openDiffFile(tab, path) {
    if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
      return;
    }
    window.cockpitFiles.openTab("file", { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, path: path });
  }

  // 「看左側版本」（spec「diff 分頁」）：from 為 EMPTY 時按鈕不顯示（renderDiffPanel() 已隱藏，這裡
  // 仍防禦一次）；為 commit 或暫存區時開某版本檔案分頁，路徑用改名時的原路徑（左側是「舊版」）。
  // from 理論上不會是 WORKTREE（design 允許的版本組合），仍防禦性地當作開檔案分頁處理。
  function openDiffLeftVersion(tab) {
    if (tab.from === "EMPTY") {
      return;
    }
    var path = tab.oldPath || tab.path;
    if (tab.from === "WORKTREE") {
      openDiffFile(tab, path);
      return;
    }
    if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
      return;
    }
    window.cockpitFiles.openTab("rev", { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, rev: tab.from, path: path });
  }

  // 「看右側版本」：to 為 WORKTREE 時開檔案分頁；為 commit 或暫存區時開某版本檔案分頁。to 不會是
  // EMPTY（design 允許的版本組合），仍防禦性地略過。
  function openDiffRightVersion(tab) {
    if (tab.to === "EMPTY") {
      return;
    }
    if (tab.to === "WORKTREE") {
      openDiffFile(tab, tab.path);
      return;
    }
    if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
      return;
    }
    window.cockpitFiles.openTab("rev", { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, rev: tab.to, path: tab.path });
  }

  function isNonEmptyString(value) {
    return typeof value === "string" && value !== "";
  }

  // 相對路徑：同 files.js 的 isRelPath()（`/` 分隔、沒有空片段、`.`／`..`）——這裡重複一份小驗證，
  // 不依賴 files.js 的私有函式（同檔頭「重複的小工具」慣例）。
  function isRelPathLike(value) {
    return (
      isNonEmptyString(value) &&
      value.split("/").every(function (part) {
        return part !== "" && part !== "." && part !== "..";
      })
    );
  }

  // 版本字面值（spec「git 端點的共同規則」）：40／64 碼小寫十六進位 hash，或 INDEX／WORKTREE／EMPTY。
  function isVersionToken(value) {
    return value === "INDEX" || value === "WORKTREE" || value === "EMPTY" || /^[0-9a-f]{40}$/.test(value) || /^[0-9a-f]{64}$/.test(value);
  }

  function createDiffTab(fields) {
    var name = baseName(fields.path);
    var iconName = typeof fields.icon === "string" && fields.icon !== "" ? fields.icon : DEFAULT_FILE_ICON;
    var label = name + " · " + sideLabel(fields.to);
    var shell = buildTabShell("diff", label, iconName);
    if (shell === null) {
      return null;
    }
    var titleParts = [fields.oldPath ? fields.oldPath + " → " + fields.path : fields.path, sideLabel(fields.from) + " → " + sideLabel(fields.to), "根目錄：" + fields.rootName];
    shell.els.tab.title = titleParts.join("\n");
    setAttr(shell.els.tab, "data-diff-path", fields.path);
    setAttr(shell.els.tab, "data-diff-from", fields.from);
    setAttr(shell.els.tab, "data-diff-to", fields.to);
    setAttr(shell.els.tab, "data-diff-old-path", fields.oldPath || null);

    var panel = shell.els.panel;
    panel.classList.add("file-panel"); // 共用檔案分頁的框、內距與 is-stale 樣式（見上方註解）

    var toolbar = document.createElement("div");
    toolbar.className = "diff-toolbar";
    var pathEl = document.createElement("span");
    pathEl.className = "diff-toolbar-path";
    var versionsEl = document.createElement("span");
    versionsEl.className = "diff-toolbar-versions";
    var staleLabel = document.createElement("span");
    staleLabel.className = "file-stale-label";
    staleLabel.textContent = "過期";
    staleLabel.hidden = true;
    var openFileBtn = document.createElement("button");
    openFileBtn.type = "button";
    openFileBtn.className = "action-button";
    openFileBtn.textContent = "開啟檔案";
    var vscodeLink = document.createElement("a");
    vscodeLink.className = "action-button file-vscode";
    vscodeLink.textContent = "在 VS Code 開啟";
    vscodeLink.hidden = true;
    var leftVersionBtn = document.createElement("button");
    leftVersionBtn.type = "button";
    leftVersionBtn.className = "action-button";
    leftVersionBtn.textContent = "看左側版本";
    var rightVersionBtn = document.createElement("button");
    rightVersionBtn.type = "button";
    rightVersionBtn.className = "action-button";
    rightVersionBtn.textContent = "看右側版本";
    toolbar.appendChild(pathEl);
    toolbar.appendChild(versionsEl);
    toolbar.appendChild(staleLabel);
    toolbar.appendChild(openFileBtn);
    toolbar.appendChild(vscodeLink);
    toolbar.appendChild(leftVersionBtn);
    toolbar.appendChild(rightVersionBtn);

    var status = document.createElement("p");
    status.className = "file-status";
    status.hidden = true;

    var host = document.createElement("div");
    host.className = "file-viewer-host"; // 重用（scrollbar-thin、flex:1 1 auto、overflow:auto）
    host.hidden = true;

    panel.appendChild(toolbar);
    panel.appendChild(status);
    panel.appendChild(host);

    var tab = {
      runtime: fields.runtime,
      rootId: fields.rootId,
      rootName: fields.rootName,
      from: fields.from,
      to: fields.to,
      path: fields.path,
      oldPath: fields.oldPath || null,
      diffStatus: "idle",
      diffCode: null,
      diffVersion: null,
      diffBody: null,
      diffGen: 0,
      diffController: null,
      workFile: null,
      workGen: 0,
      scroll: { top: 0 },
      closed: false,
      els: {
        wrap: shell.els.wrap,
        tab: shell.els.tab,
        close: shell.els.close,
        panel: panel,
        icon: shell.els.icon,
        pathEl: pathEl,
        versionsEl: versionsEl,
        staleLabel: staleLabel,
        openFileBtn: openFileBtn,
        vscodeLink: vscodeLink,
        leftVersionBtn: leftVersionBtn,
        rightVersionBtn: rightVersionBtn,
        status: status,
        host: host,
        grid: null,
      },
    };

    host.addEventListener("scroll", function () {
      if (!panel.hidden) {
        tab.scroll.top = host.scrollTop;
      }
    });
    openFileBtn.addEventListener("click", function () {
      openDiffFile(tab, tab.path);
    });
    leftVersionBtn.addEventListener("click", function () {
      openDiffLeftVersion(tab);
    });
    rightVersionBtn.addEventListener("click", function () {
      openDiffRightVersion(tab);
    });

    renderDiffPanel(tab);
    refreshDiffWorkFile(tab);
    if (!diffNeedsPolling(tab)) {
      fetchDiffOnce(tab);
    }
    return tab;
  }

  var DIFF_KIND = {
    identity: diffIdentity,
    create: createDiffTab,
    // 成為目前分頁：捲動位置寫回、工作區或暫存區側時開始輪詢（內部一定先 stopDiffPolling()）。
    activate: function (tab) {
      restoreDiffScroll(tab);
      if (diffNeedsPolling(tab)) {
        startDiffPolling(tab);
      }
    },
    // 被切走：記下捲動位置、工作區或暫存區側時停止輪詢（不管接下來要換去哪個 kind，同 files.js
    // FILE_KIND 的既有裁決：每個 kind 自己在這裡停自己的輪詢，不依賴「entering 是什麼」）。
    deactivate: function (tab) {
      captureDiffScroll(tab);
      if (diffNeedsPolling(tab)) {
        stopDiffPolling();
      }
    },
    // 分頁被關閉：世代加一（之後才回來的一次性讀取直接丟棄），中止進行中的一次性讀取（輪詢由
    // deactivate() 在切走時已經停過，關閉分頁前一定會先被切走）。
    dispose: function (tab) {
      tab.closed = true;
      tab.diffGen += 1;
      if (tab.diffController !== null) {
        tab.diffController.abort();
        tab.diffController = null;
      }
    },
    // 分頁還原（git-review task 4.3；design D9 的 v:2 格式）：內容（`diffBody`／`diffVersion`）不存
    // 還原後靠一次性讀取或輪詢重新取得，跟其餘 kind 一致（file kind 的 `meta` 也是還原後重查）。
    serialize: function (tab) {
      return { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, from: tab.from, to: tab.to, path: tab.path, oldPath: tab.oldPath };
    },
    deserialize: function (obj) {
      if (obj === null || typeof obj !== "object") {
        return null;
      }
      if (!isNonEmptyString(obj.runtime) || !isNonEmptyString(obj.rootId) || typeof obj.rootName !== "string") {
        return null;
      }
      if (!isVersionToken(obj.from) || !isVersionToken(obj.to)) {
        return null;
      }
      if (!isRelPathLike(obj.path)) {
        return null;
      }
      var oldPath = obj.oldPath === null || obj.oldPath === undefined ? null : obj.oldPath;
      if (oldPath !== null && !isRelPathLike(oldPath)) {
        return null;
      }
      return { runtime: obj.runtime, rootId: obj.rootId, rootName: obj.rootName, from: obj.from, to: obj.to, path: obj.path, oldPath: oldPath };
    },
  };

  // ---------------------------------------------------------------------------
  // Git Graph 分頁內容（git-review task 4.4；spec git-review「Git Graph 分頁」；design D8／D9）
  // ---------------------------------------------------------------------------
  //
  // 資料來源：refs 端點（分支篩選清單、ref 標籤、HEAD、輪詢偵測「分支已變更」）與 log 端點
  // （commit 清單＋`graph` 排版，見 cockpit-git/src/graph.rs 模組文件的線段語意：`half:"top"` 是
  // 列頂到節點中心、`bottom` 是節點中心到列底，`from`／`to` 是欄）。
  //
  // 前綴穩定＋分批載入（design D8）：第一批不帶 `tip`（無篩選時＝refs 端點列出的全部 ref 與
  // HEAD；有篩選時＝篩選的 ref），拿到的 `tips` 存進 `tab.tips`；之後每一批一律帶 `tip`，不再
  // 從 refs 解析——這樣中途新增 commit／移動分支都不會打亂已載入的列。一個 session（同一組
  // `tips`）內 commit 只會增加、不會重排或移除，所以列的 DOM 只增量 append，不需要 files.js
  // 檔案樹那種鍵比對（renderTree()）——這是 Graph 跟「變更」面板／檔案樹在重畫策略上唯一的差異，
  // 前提是「篩選改變／按重新整理／重新載入」一定會整個 reset（resetGraphSession()）才重新
  // 開始一個新 session。
  //
  // refs 輪詢只做「偵測分支變了沒」，不會拿新的 refs 內容去重畫已經畫好的列（design：不自動
  // 重新載入，避免打斷捲動與選取）；`tab.refsBody` 只在（重）載入時更新一次，用來畫 ref 標籤、
  // HEAD 節點與分支篩選 popover 的清單，session 期間固定不變。
  //
  // 選取／捲動狀態（memory full-repaint-discards-state-held-only-in-dom）：`tab.selectedOid`／
  // `tab.focusedIndex`／`tab.scroll.top` 都是模組（tab）變數，DOM 只是呈現；`#review` 底下的節點
  // 本來就在 `#app` 之外（design D6），整頁重畫完全碰不到，不需要額外的「重畫後寫回」邏輯（同
  // 「變更」面板／diff 分頁的既有結論）。

  var GRAPH_ROW_H = 26; // px（spec「每列固定高度」的控制端裁決：24–28px 區間）
  var GRAPH_COL_W = 18; // px：一欄的寬度（節點半徑 4px，留出可辨識的車道間距）
  var GRAPH_NODE_R = 4;
  var GRAPH_HEAD_R = 6; // HEAD 節點的外框半徑（design D8：「實心加外框」）
  var GRAPH_BATCH = 200; // 跟 log 端點的預設／最大 limit 一致
  var GRAPH_MAX_TOTAL = 5000; // spec「達 5000 筆時顯示已達上限」
  var GRAPH_LOAD_MORE_THRESHOLD_PX = 3 * GRAPH_ROW_H; // 捲到距底部這麼近時自動載入下一批
  var GRAPH_LANE_VARS = ["--accent", "--ok", "--warn", "--bad", "--graph-lane-4", "--graph-lane-5"];
  var SVG_NS = "http://www.w3.org/2000/svg";
  var GRAPH_FILTER_GROUPS = [
    { kind: "branch", title: "本地分支" },
    { kind: "remote", title: "遠端分支" },
    { kind: "tag", title: "tag" },
  ];

  function refsUrl(runtime, rootId) {
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/refs";
  }

  // commit 詳情／比較（git-review task 4.5；spec git-review「commit 詳情與比較」）用的端點網址。
  function commitUrl(runtime, rootId, oid) {
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/commit/" + seg(oid);
  }

  function changesUrl(runtime, rootId, from, to) {
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/changes?from=" + seg(from) + "&to=" + seg(to);
  }

  function mergeBaseUrl(runtime, rootId, a, b) {
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/merge-base?a=" + seg(a) + "&b=" + seg(b);
  }

  function two(n) {
    return n < 10 ? "0" + n : String(n);
  }

  // 本地時區 YYYY-MM-DD HH:mm（spec「Git Graph 分頁」）。
  function formatGraphTime(unixSeconds) {
    var d = new Date(unixSeconds * 1000);
    return d.getFullYear() + "-" + two(d.getMonth() + 1) + "-" + two(d.getDate()) + " " + two(d.getHours()) + ":" + two(d.getMinutes());
  }

  function graphLaneVar(colorIndex) {
    return GRAPH_LANE_VARS[colorIndex] !== undefined ? GRAPH_LANE_VARS[colorIndex] : "--text";
  }

  function graphColX(col) {
    return col * GRAPH_COL_W + GRAPH_COL_W / 2;
  }

  // 一列的排版畫成一個內嵌 SVG（design D8）：先畫線段（各自落在上半或下半，見 graph.rs 模組文件的
  // 線段語意），HEAD 節點另外加一圈外框（`isHead`），節點本身畫在最上層。顏色一律用 `style.stroke`／
  // `style.fill` 引用 CSS 變數（不是 `stroke=`／`fill=` 屬性字面值）：SVG 的呈現屬性走 CSS 階層，
  // `style` 內的 `var()` 會正確解析，屬性字面值不保證每個瀏覽器都會展開 `var()`。
  function buildGraphSvg(graph, trackWidth, isHead) {
    var svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("class", "graph-svg");
    svg.setAttribute("width", String(trackWidth));
    svg.setAttribute("height", String(GRAPH_ROW_H));
    svg.setAttribute("viewBox", "0 0 " + trackWidth + " " + GRAPH_ROW_H);
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("focusable", "false");
    var mid = GRAPH_ROW_H / 2;
    graph.lines.forEach(function (line) {
      var y1 = line.half === "top" ? 0 : mid;
      var y2 = line.half === "top" ? mid : GRAPH_ROW_H;
      var el = document.createElementNS(SVG_NS, "line");
      el.setAttribute("class", "graph-line");
      el.setAttribute("x1", String(graphColX(line.from)));
      el.setAttribute("y1", String(y1));
      el.setAttribute("x2", String(graphColX(line.to)));
      el.setAttribute("y2", String(y2));
      el.setAttribute("style", "stroke: var(" + graphLaneVar(line.color) + ")");
      svg.appendChild(el);
    });
    var cx = graphColX(graph.col);
    if (isHead) {
      var ring = document.createElementNS(SVG_NS, "circle");
      ring.setAttribute("class", "graph-node-head-ring");
      ring.setAttribute("cx", String(cx));
      ring.setAttribute("cy", String(mid));
      ring.setAttribute("r", String(GRAPH_HEAD_R));
      ring.setAttribute("style", "stroke: var(" + graphLaneVar(graph.color) + ")");
      svg.appendChild(ring);
    }
    var node = document.createElementNS(SVG_NS, "circle");
    node.setAttribute("class", "graph-node");
    node.setAttribute("cx", String(cx));
    node.setAttribute("cy", String(mid));
    node.setAttribute("r", String(GRAPH_NODE_R));
    node.setAttribute("style", "fill: var(" + graphLaneVar(graph.color) + ")");
    svg.appendChild(node);
    return svg;
  }

  function graphLogUrl(tab, offset, limit) {
    var q = "offset=" + offset + "&limit=" + limit;
    if (tab.tips !== null) {
      tab.tips.forEach(function (t) {
        q += "&tip=" + seg(t);
      });
    } else if (tab.selectedRefs.length > 0) {
      tab.selectedRefs.forEach(function (r) {
        q += "&ref=" + seg(r);
      });
    }
    return "/api/git/" + seg(tab.runtime) + "/" + seg(tab.rootId) + "/log?" + q;
  }

  function isHeadOid(tab, oid) {
    return !!(tab.refsBody !== null && tab.refsBody.head !== null && tab.refsBody.head.oid === oid);
  }

  function buildRefBadges(tab, oid) {
    var frag = document.createDocumentFragment();
    if (tab.refsBody === null) {
      return frag;
    }
    var headRef = tab.refsBody.head !== null && typeof tab.refsBody.head.ref === "string" ? tab.refsBody.head.ref : null;
    tab.refsBody.refs.forEach(function (r) {
      if (r.oid !== oid) {
        return;
      }
      var span = document.createElement("span");
      span.className = "graph-ref-badge";
      setAttr(span, "data-kind", r.kind);
      if (r.kind === "branch" && r.name === headRef) {
        span.classList.add("is-head-branch");
      }
      span.textContent = r.short;
      span.title = r.name;
      frag.appendChild(span);
    });
    return frag;
  }

  function ensureRowHeadRing(tab, rowObj) {
    var wantHead = isHeadOid(tab, rowObj.oid);
    if (wantHead === rowObj.isHead) {
      return;
    }
    rowObj.isHead = wantHead;
    var newSvg = buildGraphSvg(rowObj.graph, tab.trackWidth, wantHead);
    rowObj.els.row.replaceChild(newSvg, rowObj.els.svg);
    rowObj.els.svg = newSvg;
  }

  // refs（重）載入完成後，把 ref 標籤／HEAD 節點套用到已經畫出來的列（處理「log 比 refs 先回來」
  // 的競態：第一批列建立當下 `tab.refsBody` 可能還是 null，之後補上）。
  function applyRefsToAllRows(tab) {
    tab.rows.forEach(function (rowObj) {
      rowObj.els.refs.replaceChildren(buildRefBadges(tab, rowObj.oid));
      ensureRowHeadRing(tab, rowObj);
    });
  }

  function graphRowElements(tab) {
    return tab.rows.map(function (r) {
      return r.els.row;
    });
  }

  // roving tabindex（同 files.js 檔案樹 applyRovingTabindex() 的做法）：只有一列 tabindex="0"。
  function applyRovingTabindexGraph(tab) {
    var rows = graphRowElements(tab);
    var target = tab.focusedIndex >= 0 && tab.focusedIndex < rows.length ? rows[tab.focusedIndex] : rows.length > 0 ? rows[0] : null;
    rows.forEach(function (row) {
      var want = row === target ? 0 : -1;
      if (row.tabIndex !== want) {
        row.tabIndex = want;
      }
    });
  }

  function focusGraphRowAt(tab, index) {
    var rows = graphRowElements(tab);
    if (index < 0 || index >= rows.length) {
      return;
    }
    tab.focusedIndex = index;
    applyRovingTabindexGraph(tab);
    rows[index].focus();
    rows[index].scrollIntoView({ block: "nearest" });
  }

  // ---------------------------------------------------------------------------
  // commit 詳情與比較（git-review task 4.5；spec git-review「commit 詳情與比較」「某版本檔案分頁」）
  // ---------------------------------------------------------------------------
  //
  // 詳情面板是 `.graph-listbox` 內、被選取列後面的一個相鄰兄弟節點（`tab.els.detailWrap`）：選取一個
  // commit 時建立並插在該列之後；選到別的 commit 時搬到新的列後面；再點一次同一列收合（移除）。狀態
  // （`detailKind`／`detailData`／`compareBaseOid`／`compareMode`）都是 tab 的模組變數，DOM 只是呈現
  // （memory full-repaint-discards-state-held-only-in-dom；這裡雖然不是整頁重畫的情境，但同一份原則：
  // `renderGraphDetail()` 隨時可以整份重建 `detailWrap` 的內容，不需要保留跨重建的 DOM 節點——詳情面板
  // 不像 commit 清單本身有「大量列」的效能考量，也不需要保留內部捲動位置）。
  //
  // commit 詳情（`commit` 端點）以 oid 快取在模組變數（design 控制端裁決：重畫不重抓；不同 repo 的
  // oid 理論上不會撞在一起，但仍以 root_id 併入鍵，防禦性地避免極端情況下的雜湊碰撞跨 repo 汙染）。
  var commitDetailCache = new Map(); // key: rootId + "\n" + oid → commit 端點的回應本體

  function fetchCommitDetail(tab, oid) {
    var key = tab.rootId + "\n" + oid;
    var cached = commitDetailCache.get(key);
    if (cached !== undefined) {
      return Promise.resolve({ ok: true, body: cached });
    }
    return getJson(commitUrl(tab.runtime, tab.rootId, oid)).then(function (result) {
      if (result.ok && result.body && typeof result.body.oid === "string" && Array.isArray(result.body.files)) {
        commitDetailCache.set(key, result.body);
        return { ok: true, body: result.body };
      }
      return { ok: false, code: result.ok ? "unknown" : result.code };
    });
  }

  function fetchMergeBase(tab, a, b) {
    return getJson(mergeBaseUrl(tab.runtime, tab.rootId, a, b)).then(function (result) {
      if (result.ok && result.body && typeof result.body.oid === "string") {
        return { ok: true, oid: result.body.oid };
      }
      return { ok: false, code: result.ok ? "unknown" : result.code };
    });
  }

  // --- 複製（spec：使用瀏覽器剪貼簿，成功或失敗都以文字提示回饋；控制端裁決：aria-live="polite"
  // 區域呈現，2 秒後消失，不得用 alert）---

  var copyFeedbackTimer = null; // 單例：同一時間只顯示一則提示（同 diffPoll 等單例狀態的既有裁決）

  function setCopyFeedbackText(tab, text) {
    var el = tab.els.copyFeedback;
    if (el === undefined || el === null) {
      return;
    }
    setHidden(el, text === null);
    if (text !== null && el.textContent !== text) {
      el.textContent = text;
    }
  }

  function showCopyFeedback(tab, ok) {
    if (copyFeedbackTimer !== null) {
      clearTimeout(copyFeedbackTimer.timer);
      setCopyFeedbackText(copyFeedbackTimer.tab, null);
    }
    setCopyFeedbackText(tab, ok ? "已複製" : "無法複製");
    var timer = setTimeout(function () {
      setCopyFeedbackText(tab, null);
      copyFeedbackTimer = null;
    }, 2000);
    copyFeedbackTimer = { tab: tab, timer: timer };
  }

  function copyToClipboard(tab, text) {
    if (!navigator.clipboard || typeof navigator.clipboard.writeText !== "function") {
      showCopyFeedback(tab, false);
      return;
    }
    navigator.clipboard.writeText(text).then(
      function () {
        showCopyFeedback(tab, true);
      },
      function () {
        showCopyFeedback(tab, false);
      }
    );
  }

  // --- 詳情面板：建立／搬動／收合 ---

  function collapseGraphDetail(tab) {
    if (tab.els.detailWrap !== null) {
      tab.els.detailWrap.remove();
      tab.els.detailWrap = null;
    }
    tab.detailAnchorOid = null;
    tab.detailFocusLost = null;
    tab.detailKind = null;
    tab.detailData = null;
    tab.detailStatus = "idle";
    tab.detailCode = null;
    tab.detailReqGen += 1; // 作廢還在飛的詳情／比較讀取
  }

  // 確保詳情面板緊接在 rowObj 後面；已經在那裡就直接沿用（同一個 DOM 節點，renderGraphDetail() 再
  // 重建內容）。
  function ensureGraphDetailWrap(tab, rowObj) {
    if (tab.els.detailWrap !== null && tab.detailAnchorOid === rowObj.oid) {
      return tab.els.detailWrap;
    }
    if (tab.els.detailWrap !== null) {
      tab.els.detailWrap.remove();
    }
    var wrap = document.createElement("div");
    wrap.className = "commit-detail";
    wrap.tabIndex = -1; // ui-fixes task 4.8：重建後找不到對應元素時，焦點退到容器（不落到 body）
    tab.detailFocusLost = null;
    rowObj.els.row.insertAdjacentElement("afterend", wrap);
    tab.els.detailWrap = wrap;
    tab.detailAnchorOid = rowObj.oid;
    return wrap;
  }

  function commitDetailFieldRow(labelText, valueNode) {
    var row = document.createElement("div");
    row.className = "commit-detail-row";
    var label = document.createElement("span");
    label.className = "commit-detail-label";
    label.textContent = labelText;
    row.appendChild(label);
    row.appendChild(valueNode);
    return row;
  }

  // ui-fixes task 4.8：詳情內可聚焦元素的穩定身分（元素種類＋檔案路徑／ref 名稱／oid／切換名稱）。
  // 詳情隨時整份重建，重建前後靠它找回「代表同一個對象」的元素（renderGraphDetail）。
  function setFocusKey(el, key) {
    el.setAttribute("data-focus-key", key);
  }

  function commitDetailCopyButton(tab, text, label) {
    var btn = document.createElement("button");
    btn.type = "button";
    btn.className = "action-button";
    setFocusKey(btn, "copy:" + label);
    btn.textContent = "複製";
    btn.setAttribute("aria-label", "複製" + label);
    btn.addEventListener("click", function () {
      copyToClipboard(tab, text);
    });
    return btn;
  }

  // parents（spec：短 hash，點選後捲到並選取該 commit；不在已載入範圍時顯示為純文字）：這個分支
  // 極難在驗收腳本裡穩定重現（滑到最後一列本身就會觸發「捲到底部自動載入下一批」，載入完成後
  // parent 反而變成已載入——見 task 4.5 報告「自我審查」），為了不落成 CL1 死規則（同 git.js
  // 「變更面板」既有裁決：只有這裡用得到的選擇器改用行內樣式，見檔頭「重要教訓」），不新增專屬
  // class，直接引用 --text-dim（唯一的視覺要求）。
  function commitParentNode(tab, oid) {
    var rowObj = tab.rowByOid.get(oid);
    if (rowObj === undefined) {
      var span = document.createElement("span");
      span.style.fontFamily = "var(--font-mono)";
      span.style.color = "var(--text-dim)";
      span.textContent = shortHash(oid) + "（不在已載入範圍）";
      return span;
    }
    var btn = document.createElement("button");
    btn.type = "button";
    btn.className = "action-button";
    setFocusKey(btn, "parent:" + oid);
    btn.textContent = shortHash(oid);
    btn.addEventListener("click", function () {
      jumpToGraphRow(tab, oid);
    });
    return btn;
  }

  function jumpToGraphRow(tab, oid) {
    var rowObj = tab.rowByOid.get(oid);
    if (rowObj === undefined) {
      return;
    }
    rowObj.els.row.scrollIntoView({ block: "nearest" });
    selectGraphRow(tab, rowObj, true);
  }

  // 變更檔案清單的一列（spec：狀態字母、icon、路徑、改名時的原路徑、增刪行數）；`onOpenVersion` 省略
  // 時不顯示「看此版本」（比較模式沒有這顆按鈕，見 spec「commit 詳情與比較」正文段落順序）。
  function commitDetailFileRow(entry, onOpenDiff, onOpenVersion) {
    var row = document.createElement("div");
    row.className = "commit-detail-file-row";
    row.setAttribute("role", "button");
    row.tabIndex = 0;
    setFocusKey(row, "file:" + entry.path);
    row.title = entry.path;
    var icon = document.createElement("img");
    icon.className = "tree-icon";
    icon.alt = "";
    icon.width = 16;
    icon.height = 16;
    icon.draggable = false;
    icon.src = ICON_BASE + seg(typeof entry.icon === "string" && entry.icon !== "" ? entry.icon : DEFAULT_FILE_ICON);
    var status = document.createElement("span");
    status.className = "changes-status"; // 重用「變更」面板既有的狀態字母樣式
    status.textContent = entry.status;
    status.style.color = Object.prototype.hasOwnProperty.call(STATUS_COLOR, entry.status) ? STATUS_COLOR[entry.status] : "";
    var name = document.createElement("span");
    name.className = "tree-name";
    name.textContent = entry.old_path ? entry.old_path + " → " + entry.path : entry.path;
    var counts = document.createElement("span");
    counts.className = "changes-dir"; // 重用（次要文字樣式）：增刪行數
    counts.textContent = typeof entry.additions === "number" && typeof entry.deletions === "number" ? "+" + entry.additions + " / -" + entry.deletions : "";
    row.appendChild(icon);
    row.appendChild(status);
    row.appendChild(name);
    row.appendChild(counts);
    if (typeof onOpenVersion === "function") {
      var viewBtn = document.createElement("button");
      viewBtn.type = "button";
      viewBtn.className = "action-button";
      setFocusKey(viewBtn, "view:" + entry.path);
      viewBtn.textContent = "看此版本";
      viewBtn.addEventListener("click", function (event) {
        event.stopPropagation();
        onOpenVersion(entry);
      });
      row.appendChild(viewBtn);
    }
    row.addEventListener("click", function () {
      onOpenDiff(entry);
    });
    row.addEventListener("keydown", function (event) {
      if (event.target !== row) {
        return; // 內嵌按鈕（看此版本）自己的 Enter／Space 交給按鈕原生 click，不被整列攔走
      }
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        onOpenDiff(entry);
      }
    });
    return row;
  }

  function openGraphFileDiff(tab, from, to, entry) {
    if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
      return;
    }
    window.cockpitFiles.openTab("diff", {
      runtime: tab.runtime,
      rootId: tab.rootId,
      rootName: tab.rootName,
      from: from,
      to: to,
      path: entry.path,
      oldPath: entry.old_path || null,
      icon: entry.icon,
    });
  }

  // 「看此版本」（spec：開啟該 commit 版本的某版本檔案分頁；刪除的檔案改開 compared_to 版本）。
  function openCommitFileVersion(tab, commitBody, entry) {
    var rev = entry.status === "D" ? commitBody.compared_to : commitBody.oid;
    if (rev === null || typeof rev !== "string") {
      return; // compared_to 為 null（根 commit）且檔案又是刪除狀態：理論上不會同時發生，防禦略過
    }
    if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
      return;
    }
    window.cockpitFiles.openTab("rev", { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, rev: rev, path: entry.path, icon: entry.icon });
  }

  function renderSingleCommitDetail(tab, wrap) {
    if (tab.detailStatus === "loading") {
      var loading = document.createElement("p");
      loading.className = "file-status";
      loading.textContent = "正在讀取 commit 詳情…";
      wrap.appendChild(loading);
      return;
    }
    if (tab.detailStatus === "error" || tab.detailData === null) {
      var err = document.createElement("p");
      err.className = "file-status";
      setAttr(err, "data-tone", "warn");
      err.textContent = gitErrorText(tab.detailCode);
      wrap.appendChild(err);
      return;
    }
    var body = tab.detailData;

    var hashRow = document.createElement("div");
    hashRow.className = "commit-detail-row";
    var hashLabel = document.createElement("span");
    hashLabel.className = "commit-detail-label";
    hashLabel.textContent = "hash";
    var hashValue = document.createElement("span");
    hashValue.className = "commit-detail-hash";
    hashValue.textContent = body.oid;
    hashRow.appendChild(hashLabel);
    hashRow.appendChild(hashValue);
    hashRow.appendChild(commitDetailCopyButton(tab, body.oid, "完整 hash"));
    wrap.appendChild(hashRow);

    wrap.appendChild(commitDetailFieldRow("作者", document.createTextNode(body.author.name + " <" + body.author.email + ">")));
    wrap.appendChild(commitDetailFieldRow("時間", document.createTextNode(formatGraphTime(body.author.time))));
    if (body.committer && (body.committer.name !== body.author.name || body.committer.email !== body.author.email)) {
      wrap.appendChild(commitDetailFieldRow("committer", document.createTextNode(body.committer.name + " <" + body.committer.email + ">")));
    }

    if (Array.isArray(body.parents) && body.parents.length > 0) {
      var parentsRow = document.createElement("div");
      parentsRow.className = "commit-detail-row";
      var parentsLabel = document.createElement("span");
      parentsLabel.className = "commit-detail-label";
      parentsLabel.textContent = "parents";
      parentsRow.appendChild(parentsLabel);
      body.parents.forEach(function (p) {
        parentsRow.appendChild(commitParentNode(tab, p));
      });
      wrap.appendChild(parentsRow);
    }

    var pointingRefs = tab.refsBody !== null ? tab.refsBody.refs.filter(function (r) { return r.oid === body.oid; }) : [];
    if (pointingRefs.length > 0) {
      var refsRow = document.createElement("div");
      refsRow.className = "commit-detail-row";
      var refsLabel = document.createElement("span");
      refsLabel.className = "commit-detail-label";
      refsLabel.textContent = "指向它的 ref";
      refsRow.appendChild(refsLabel);
      pointingRefs.forEach(function (r) {
        var badge = document.createElement("span");
        badge.className = "graph-ref-badge"; // 重用既有 ref 標籤樣式
        setAttr(badge, "data-kind", r.kind);
        badge.textContent = r.short;
        refsRow.appendChild(badge);
        refsRow.appendChild(commitDetailCopyButton(tab, r.short, "「" + r.short + "」"));
      });
      wrap.appendChild(refsRow);
    }

    var message = document.createElement("p");
    message.className = "commit-detail-message";
    message.textContent = body.message;
    wrap.appendChild(message);

    var actions = document.createElement("div");
    actions.className = "commit-detail-actions";
    var baseBtn = document.createElement("button");
    baseBtn.type = "button";
    baseBtn.className = "action-button";
    setFocusKey(baseBtn, "compare-base");
    baseBtn.textContent = "選為比較基準";
    baseBtn.addEventListener("click", function () {
      tab.compareBaseOid = body.oid;
      renderGraphDetail(tab); // 立即反映；真正進入比較畫面要等使用者選第二個 commit
    });
    actions.appendChild(baseBtn);
    if (tab.compareBaseOid === body.oid) {
      var baseNote = document.createElement("span");
      baseNote.className = "tree-note";
      baseNote.textContent = "已選為比較基準，點選另一個 commit 開始比較";
      actions.appendChild(baseNote);
    }
    wrap.appendChild(actions);

    var filesHead = document.createElement("div");
    filesHead.className = "commit-detail-row";
    var filesLabel = document.createElement("span");
    filesLabel.className = "commit-detail-label";
    filesLabel.textContent = "變更檔案（" + body.files.length + "）";
    filesHead.appendChild(filesLabel);
    if (Array.isArray(body.parents) && body.parents.length > 1) {
      var mergeNote = document.createElement("span");
      mergeNote.className = "tree-note";
      mergeNote.textContent = "與第一個父 commit 比較";
      filesHead.appendChild(mergeNote);
    }
    wrap.appendChild(filesHead);

    var filesList = document.createElement("div");
    filesList.className = "commit-detail-files";
    body.files.forEach(function (entry) {
      filesList.appendChild(
        commitDetailFileRow(
          entry,
          function (e) {
            openGraphFileDiff(tab, body.compared_to === null ? "EMPTY" : body.compared_to, body.oid, e);
          },
          function (e) {
            openCommitFileVersion(tab, body, e);
          }
        )
      );
    });
    wrap.appendChild(filesList);
    if (body.truncated === true) {
      wrap.appendChild(truncatedFilesNote());
    }
  }

  // ui-fixes task 4.7：檔案清單被後端截斷（`truncated` 為真）時，在清單末端顯示的提示；文案與樣式
  // （.tree-note）同變更清單的 note row（desiredChangeRows），詳情與比較清單不是虛擬清單，所以另建元素。
  function truncatedFilesNote() {
    var note = document.createElement("div");
    note.className = "tree-note";
    note.textContent = "變更過多，只列出前面一部分";
    return note;
  }

  function renderCompareDetail(tab, wrap) {
    var data = tab.detailData;

    var header = document.createElement("div");
    header.className = "commit-detail-row";
    var headerLabel = document.createElement("span");
    headerLabel.className = "commit-detail-label";
    headerLabel.textContent = "比較 " + shortHash(data.baseOid) + " ↔ " + shortHash(data.targetOid);
    header.appendChild(headerLabel);
    wrap.appendChild(header);

    var toggle = document.createElement("div");
    toggle.className = "commit-detail-actions";
    var directBtn = document.createElement("button");
    directBtn.type = "button";
    directBtn.className = "action-button";
    setFocusKey(directBtn, "toggle:direct");
    directBtn.textContent = "直接比較";
    directBtn.setAttribute("aria-pressed", tab.compareMode === "direct" ? "true" : "false");
    directBtn.addEventListener("click", function () {
      if (tab.compareMode !== "direct") {
        tab.compareMode = "direct";
        loadGraphCompareFiles(tab, data.baseOid, data.targetOid);
      }
    });
    var forkBtn = document.createElement("button");
    forkBtn.type = "button";
    forkBtn.className = "action-button";
    setFocusKey(forkBtn, "toggle:fork");
    forkBtn.textContent = "自分岔點起";
    forkBtn.setAttribute("aria-pressed", tab.compareMode === "fork" ? "true" : "false");
    forkBtn.addEventListener("click", function () {
      if (tab.compareMode !== "fork") {
        tab.compareMode = "fork";
        loadGraphCompareFiles(tab, data.baseOid, data.targetOid);
      }
    });
    toggle.appendChild(directBtn);
    toggle.appendChild(forkBtn);
    wrap.appendChild(toggle);

    if (tab.detailStatus === "loading") {
      var loading = document.createElement("p");
      loading.className = "file-status";
      loading.textContent = "正在讀取變更…";
      wrap.appendChild(loading);
      return;
    }
    if (tab.detailStatus === "error") {
      var err = document.createElement("p");
      err.className = "file-status";
      setAttr(err, "data-tone", "warn");
      err.textContent = gitErrorText(tab.detailCode);
      wrap.appendChild(err);
      return;
    }

    var filesHead = document.createElement("div");
    filesHead.className = "commit-detail-row";
    var filesLabel = document.createElement("span");
    filesLabel.className = "commit-detail-label";
    filesLabel.textContent = "變更檔案（" + data.files.length + "）";
    filesHead.appendChild(filesLabel);
    wrap.appendChild(filesHead);

    var filesList = document.createElement("div");
    filesList.className = "commit-detail-files";
    data.files.forEach(function (entry) {
      filesList.appendChild(
        commitDetailFileRow(entry, function (e) {
          openGraphFileDiff(tab, data.effectiveBase, data.targetOid, e);
        })
      );
    });
    wrap.appendChild(filesList);
    if (data.truncated === true) {
      wrap.appendChild(truncatedFilesNote());
    }
  }

  function renderGraphDetail(tab) {
    if (tab.els.detailWrap === null) {
      return;
    }
    var wrap = tab.els.detailWrap;
    // ui-fixes task 4.8（design D7）：重建前若焦點在詳情內，記下身分；重建後找回對應元素，經共用的
    // 焦點 helper 還原（最近輸入是滑鼠就不呈現焦點外框）。找不到時聚焦容器，不落到 body，並把身分留在
    // tab 上：載入中的畫面還沒有檔案列，載入完成的下一次重建才找得回來。焦點本來就不在詳情內
    // （使用者已移到別處）時不搶焦點。
    var active = document.activeElement;
    var focusKey = null;
    var hadFocus = false;
    // 修正波 1 F-M1：舊焦點元素原本匹配 :focus-visible（鍵盤使用者 Tab 到的）就保留外框。
    var keepVisible =
      !!window.cockpitFocus &&
      typeof window.cockpitFocus.focusVisible === "function" &&
      window.cockpitFocus.focusVisible(active);
    if (active === wrap) {
      hadFocus = true;
      focusKey = tab.detailFocusLost;
    } else if (active !== null && wrap.contains(active)) {
      hadFocus = true;
      focusKey = active.getAttribute("data-focus-key");
    }
    wrap.replaceChildren();
    if (tab.detailKind === "single") {
      renderSingleCommitDetail(tab, wrap);
    } else if (tab.detailKind === "compare") {
      renderCompareDetail(tab, wrap);
    }
    tab.detailFocusLost = null;
    if (!hadFocus) {
      return;
    }
    var target = null;
    if (focusKey) {
      var keyed = wrap.querySelectorAll("[data-focus-key]");
      for (var i = 0; i < keyed.length; i += 1) {
        if (keyed[i].getAttribute("data-focus-key") === focusKey) {
          target = keyed[i];
          break;
        }
      }
    }
    if (target === null) {
      tab.detailFocusLost = focusKey || null;
      target = wrap;
    }
    if (window.cockpitFocus && typeof window.cockpitFocus.focus === "function") {
      window.cockpitFocus.focus(target, keepVisible);
    } else {
      target.focus({ preventScroll: true });
    }
  }

  function openGraphSingleDetail(tab, oid) {
    var rowObj = tab.rowByOid.get(oid);
    if (rowObj === undefined) {
      return;
    }
    tab.detailKind = "single";
    ensureGraphDetailWrap(tab, rowObj);
    tab.detailStatus = "loading";
    tab.detailCode = null;
    tab.detailData = null;
    renderGraphDetail(tab);
    var gen = (tab.detailReqGen += 1);
    fetchCommitDetail(tab, oid).then(function (result) {
      if (tab.closed || gen !== tab.detailReqGen || tab.detailKind !== "single" || tab.detailAnchorOid !== oid) {
        return;
      }
      if (result.ok) {
        tab.detailStatus = "ok";
        tab.detailData = result.body;
      } else {
        tab.detailStatus = "error";
        tab.detailCode = result.code;
      }
      renderGraphDetail(tab);
    });
  }

  function loadGraphCompareFiles(tab, baseOid, targetOid) {
    var gen = (tab.detailReqGen += 1);
    tab.detailStatus = "loading";
    tab.detailCode = null;
    renderGraphDetail(tab);
    var basePromise = tab.compareMode === "fork" ? fetchMergeBase(tab, baseOid, targetOid) : Promise.resolve({ ok: true, oid: baseOid });
    basePromise.then(function (baseResult) {
      if (tab.closed || gen !== tab.detailReqGen) {
        return;
      }
      if (!baseResult.ok) {
        tab.detailStatus = "error";
        tab.detailCode = baseResult.code;
        tab.detailData = { baseOid: baseOid, targetOid: targetOid, effectiveBase: null, files: [] };
        renderGraphDetail(tab);
        return;
      }
      var effectiveBase = baseResult.oid;
      getJson(changesUrl(tab.runtime, tab.rootId, effectiveBase, targetOid)).then(function (result) {
        if (tab.closed || gen !== tab.detailReqGen) {
          return;
        }
        if (result.ok && result.body && Array.isArray(result.body.files)) {
          tab.detailStatus = "ok";
          tab.detailCode = null;
          tab.detailData = { baseOid: baseOid, targetOid: targetOid, effectiveBase: effectiveBase, files: result.body.files, truncated: result.body.truncated === true };
        } else {
          tab.detailStatus = "error";
          tab.detailCode = result.ok ? "unknown" : result.code;
          tab.detailData = { baseOid: baseOid, targetOid: targetOid, effectiveBase: effectiveBase, files: [] };
        }
        renderGraphDetail(tab);
      });
    });
  }

  function openGraphCompareDetail(tab, baseOid, targetOid) {
    var rowObj = tab.rowByOid.get(targetOid);
    if (rowObj === undefined) {
      return;
    }
    tab.detailKind = "compare";
    // 先放一份空的比較資料：載入中標頭要讀 baseOid／targetOid，且前一個詳情的 detailData 形狀不同（或為 null）
    tab.detailData = { baseOid: baseOid, targetOid: targetOid, effectiveBase: null, files: [] };
    tab.compareMode = "direct"; // 每次新配對的比較都從「直接比較」開始（控制端裁決：最少驚訝）
    ensureGraphDetailWrap(tab, rowObj);
    loadGraphCompareFiles(tab, baseOid, targetOid);
  }

  function cancelGraphCompareBase(tab) {
    if (tab.compareBaseOid === null) {
      return;
    }
    tab.compareBaseOid = null;
    if (tab.detailKind === "compare" && tab.detailAnchorOid !== null) {
      openGraphSingleDetail(tab, tab.detailAnchorOid);
    }
  }

  // 選取改變（selectGraphRow() 在真的換選取時呼叫一次）：已有比較基準且新選取不是它自己時，展開
  // 「比較」詳情；否則展開單一 commit 的詳情（spec「commit 詳情與比較」）。
  function onCommitSelected(tab, oid) {
    if (tab.compareBaseOid !== null && tab.compareBaseOid !== oid) {
      openGraphCompareDetail(tab, tab.compareBaseOid, oid);
    } else {
      openGraphSingleDetail(tab, oid);
    }
  }

  function selectGraphRow(tab, rowObj, alsoFocus) {
    if (tab.selectedOid !== rowObj.oid) {
      if (tab.selectedOid !== null) {
        var prev = tab.rowByOid.get(tab.selectedOid);
        if (prev !== undefined) {
          prev.els.row.classList.remove("is-selected");
          prev.els.row.setAttribute("aria-selected", "false");
        }
      }
      tab.selectedOid = rowObj.oid;
      rowObj.els.row.classList.add("is-selected");
      rowObj.els.row.setAttribute("aria-selected", "true");
      onCommitSelected(tab, rowObj.oid);
    }
    var idx = tab.rows.indexOf(rowObj);
    if (idx >= 0) {
      tab.focusedIndex = idx;
      applyRovingTabindexGraph(tab);
    }
    if (alsoFocus) {
      rowObj.els.row.focus();
    }
  }

  // 鍵盤（spec「Git Graph 分頁」：上下方向鍵移動、Enter／Space 選取；同 files.js 檔案樹
  // onTreeKeydown() 的「焦點移動」與「選取」分離模式）。
  function onGraphListKeydown(tab, event) {
    var row = event.target instanceof Element ? event.target.closest(".graph-row") : null;
    if (row === null) {
      return;
    }
    var rows = graphRowElements(tab);
    var index = rows.indexOf(row);
    if (index < 0) {
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusGraphRowAt(tab, Math.min(index + 1, rows.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusGraphRowAt(tab, Math.max(index - 1, 0));
    } else if (event.key === "Home") {
      event.preventDefault();
      focusGraphRowAt(tab, 0);
    } else if (event.key === "End") {
      event.preventDefault();
      focusGraphRowAt(tab, rows.length - 1);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectGraphRow(tab, tab.rows[index], false);
    }
  }

  function createGraphRowEl(tab, row) {
    var el = document.createElement("div");
    el.className = "graph-row";
    el.setAttribute("role", "option");
    el.tabIndex = -1;
    setAttr(el, "data-oid", row.oid);
    el.setAttribute("aria-selected", "false");

    var isHead = isHeadOid(tab, row.oid);
    var svg = buildGraphSvg(row.graph, tab.trackWidth, isHead);

    var refsEl = document.createElement("span");
    refsEl.className = "graph-refs";
    refsEl.appendChild(buildRefBadges(tab, row.oid));

    var subjectEl = document.createElement("span");
    subjectEl.className = "graph-subject";
    subjectEl.textContent = row.subject;
    subjectEl.title = row.subject;

    var messageEl = document.createElement("span");
    messageEl.className = "graph-message";
    messageEl.appendChild(refsEl);
    messageEl.appendChild(subjectEl);

    var authorEl = document.createElement("span");
    authorEl.className = "graph-author";
    authorEl.textContent = row.author;
    authorEl.title = row.author + " <" + row.email + ">";

    var timeEl = document.createElement("span");
    timeEl.className = "graph-time";
    timeEl.textContent = formatGraphTime(row.time);

    var hashEl = document.createElement("span");
    hashEl.className = "graph-hash";
    hashEl.textContent = shortHash(row.oid);
    hashEl.title = row.oid;

    el.appendChild(svg);
    el.appendChild(messageEl);
    el.appendChild(authorEl);
    el.appendChild(timeEl);
    el.appendChild(hashEl);

    var rowObj = {
      oid: row.oid,
      parents: row.parents,
      author: row.author,
      email: row.email,
      time: row.time,
      subject: row.subject,
      graph: row.graph,
      isHead: isHead,
      els: { row: el, svg: svg, refs: refsEl, subject: subjectEl },
    };
    el.addEventListener("click", function (event) {
      onGraphRowClick(tab, rowObj, event);
    });
    return rowObj;
  }

  // 滑鼠點列（spec「commit 詳情與比較」；控制端裁決：Ctrl／⌘＋點選直接指定第二個 commit、再點一次
  // 已展開詳情的列收合）。鍵盤 Enter／Space（onGraphListKeydown()）不重現這兩個手勢，只呼叫
  // selectGraphRow()——這是控制端裁決，鍵盤路徑維持單純的選取語意。
  function onGraphRowClick(tab, rowObj, event) {
    var ctrlLike = !!(event && (event.ctrlKey || event.metaKey));
    if (ctrlLike && tab.selectedOid !== null && tab.selectedOid !== rowObj.oid) {
      tab.compareBaseOid = tab.selectedOid;
      selectGraphRow(tab, rowObj, true);
      return;
    }
    if (tab.selectedOid === rowObj.oid && tab.detailAnchorOid === rowObj.oid) {
      collapseGraphDetail(tab);
      return;
    }
    selectGraphRow(tab, rowObj, true);
    // selectGraphRow() 只在「選取真的改變」時才呼叫 onCommitSelected()；再點一次同一列讓詳情從收合
    // 狀態重新展開時（oid 沒變），這裡補呼叫一次，是 toggle 的「開」那一半。
    if (tab.detailAnchorOid !== rowObj.oid) {
      onCommitSelected(tab, rowObj.oid);
    }
  }

  // 既有列的 SVG 只需要放寬 width／viewBox：欄位座標（graphColX()）不受 trackWidth 影響，既有的
  // 線段／節點元素完全不用重建。
  function widenExistingRowSvgs(tab) {
    tab.rows.forEach(function (rowObj) {
      rowObj.els.svg.setAttribute("width", String(tab.trackWidth));
      rowObj.els.svg.setAttribute("viewBox", "0 0 " + tab.trackWidth + " " + GRAPH_ROW_H);
    });
  }

  // 只增量 append（見本節開頭「前綴穩定＋分批載入」的說明：一個 session 內 commit 只會增加）。
  function appendGraphRows(tab, rows) {
    rows.forEach(function (row) {
      if (tab.rowByOid.has(row.oid)) {
        return; // 防禦：理論上前綴穩定不會重複，仍不假設伺服器輸出完美
      }
      var rowMaxCol = row.graph.col;
      row.graph.lines.forEach(function (l) {
        rowMaxCol = Math.max(rowMaxCol, l.from, l.to);
      });
      if (rowMaxCol > tab.maxCol) {
        tab.maxCol = rowMaxCol;
        var newWidth = (tab.maxCol + 1) * GRAPH_COL_W;
        if (newWidth > tab.trackWidth) {
          tab.trackWidth = newWidth;
          widenExistingRowSvgs(tab);
        }
      }
      var rowObj = createGraphRowEl(tab, row);
      tab.rows.push(rowObj);
      tab.rowByOid.set(row.oid, rowObj);
      tab.els.listbox.appendChild(rowObj.els.row);
    });
    applyRovingTabindexGraph(tab);
  }

  function updateLoadMoreVisibility(tab) {
    setHidden(tab.els.loadMoreBtn, !tab.hasMore);
    setHidden(tab.els.capNote, !tab.atCap);
  }

  function isGraphTabActive(tab) {
    return tab.els.panel !== null && tab.els.panel.hidden === false;
  }

  function renderGraphStatus(tab) {
    var text = null;
    var tone = "dim";
    if (tab.loadStatus === "error") {
      text = gitErrorText(tab.loadCode);
      tone = "warn";
    } else if (tab.loadStatus === "loading" && tab.rows.length === 0) {
      text = "正在讀取 commit…";
    }
    setHidden(tab.els.status, text === null);
    if (text !== null && tab.els.status.textContent !== text) {
      tab.els.status.textContent = text;
    }
    setAttr(tab.els.status, "data-tone", tone);
    setHidden(tab.els.emptyNote, !(tab.loadStatus === "ok" && tab.rows.length === 0));
  }

  function renderGraphBanner(tab) {
    setHidden(tab.els.banner, !tab.refsChanged);
  }

  // 捲到距底部 GRAPH_LOAD_MORE_THRESHOLD_PX 內（或清單本來就沒填滿容器）時自動載入下一批。
  function maybeAutoLoadMore(tab) {
    if (tab.closed || !tab.hasMore || tab.loadStatus === "loading") {
      return;
    }
    var el = tab.els.scroller;
    // 非目前分頁（panel 隱藏）時三個量都是 0，「0-0-0 <= 門檻」會誤判成沒填滿而一路載到上限；
    // 切回時由 GRAPH_KIND.activate() 再呼叫一次。
    if (!isGraphTabActive(tab) || el.clientHeight === 0) {
      return;
    }
    if (el.scrollHeight - el.scrollTop - el.clientHeight <= GRAPH_LOAD_MORE_THRESHOLD_PX) {
      loadGraphBatch(tab, tab.rows.length);
    }
  }

  function loadGraphBatch(tab, offset) {
    tab.loadStatus = "loading";
    renderGraphStatus(tab);
    var gen = (tab.loadGen += 1);
    var controller = new AbortController();
    tab.loadController = controller;
    getJson(graphLogUrl(tab, offset, GRAPH_BATCH), controller.signal).then(function (result) {
      if (tab.closed || gen !== tab.loadGen) {
        return;
      }
      tab.loadController = null;
      if (!result.ok || !result.body || !Array.isArray(result.body.rows)) {
        tab.loadStatus = "error";
        tab.loadCode = result.ok ? "unknown" : result.code;
        renderGraphStatus(tab);
        return;
      }
      tab.loadStatus = "ok";
      tab.loadCode = null;
      if (tab.tips === null) {
        tab.tips = Array.isArray(result.body.tips) ? result.body.tips : [];
      }
      appendGraphRows(tab, result.body.rows);
      tab.hasMore = result.body.has_more === true;
      tab.atCap = tab.rows.length >= GRAPH_MAX_TOTAL;
      renderGraphStatus(tab);
      updateLoadMoreVisibility(tab);
      if (tab.searchQuery !== "") {
        recomputeGraphSearch(tab, true);
      }
      maybeAutoLoadMore(tab);
    });
  }

  function fetchGraphRefs(tab) {
    return getJson(refsUrl(tab.runtime, tab.rootId)).then(function (result) {
      if (tab.closed) {
        return null;
      }
      if (result.ok && result.body && Array.isArray(result.body.refs)) {
        tab.refsBody = result.body;
        rebuildGraphFilterPopover(tab);
        return result.body;
      }
      return null;
    });
  }

  function resetGraphSession(tab) {
    tab.loadGen += 1; // 丟棄還在飛的分批讀取
    if (tab.loadController !== null) {
      tab.loadController.abort();
      tab.loadController = null;
    }
    tab.rows = [];
    tab.rowByOid = new Map();
    tab.tips = null;
    tab.hasMore = false;
    tab.atCap = false;
    tab.maxCol = 0;
    tab.trackWidth = GRAPH_COL_W;
    tab.selectedOid = null;
    tab.focusedIndex = -1;
    tab.searchMatches = [];
    tab.searchIndex = -1;
    tab.refsChanged = false;
    tab.loadStatus = "idle";
    tab.loadCode = null;
    // git-review task 4.5：commit 詳情／比較狀態同 session 一起重設（listbox.replaceChildren() 已經
    // 把詳情面板從 DOM 移除，這裡把 tab 的模組變數也歸零，避免下一個 session 誤以為詳情還開著）。
    tab.compareBaseOid = null;
    tab.compareMode = "direct";
    tab.detailKind = null;
    tab.detailAnchorOid = null;
    tab.detailData = null;
    tab.detailStatus = "idle";
    tab.detailCode = null;
    tab.detailReqGen += 1;
    tab.els.detailWrap = null;
    tab.els.listbox.replaceChildren();
    tab.scroll.top = 0;
    tab.els.scroller.scrollTop = 0;
  }

  // 從頭重新載入（初次建立、分支篩選套用後、工具列「重新整理」、banner「重新載入」共用這一個
  // 入口；design D8：banner「不自動重新載入」，只有這幾個明確的使用者動作才會呼叫它）。
  function loadInitialGraph(tab) {
    resetGraphSession(tab);
    renderGraphStatus(tab);
    renderGraphBanner(tab);
    updateLoadMoreVisibility(tab);
    fetchGraphRefs(tab).then(function () {
      applyRefsToAllRows(tab);
    });
    loadGraphBatch(tab, 0);
    if (isGraphTabActive(tab)) {
      startGraphRefsPolling(tab);
    }
  }

  // refs 輪詢：偵測「分支已變更」用的假想 tips 計算（同 cockpit::git log_inner() 的起點規則，
  // 純為了跟 tab.tips 比對，不會真的拿去打 log 端點）——有篩選時＝各篩選 ref 目前的 oid（依
  // selectedRefs 的既定順序）；沒有篩選時＝refs 清單去重後的 oid＋HEAD（不在清單中才附加）。
  // ui-fixes task 4.6（design D4）：沒有篩選時後端起點只收 `commit` 為真的 ref（指向 tree／blob 的 tag
  // 不算），這裡必須同一條規則，否則 tips 永遠對不上、「分支已變更」永久誤報；有篩選時後端照篩選名取
  // oid、不看 `commit`，這裡同樣不看。
  function computeExpectedTips(refsBody, selectedRefNames) {
    if (selectedRefNames.length > 0) {
      return selectedRefNames.map(function (name) {
        var found = null;
        refsBody.refs.forEach(function (r) {
          if (r.name === name) {
            found = r.oid;
          }
        });
        return found;
      });
    }
    var seen = {};
    var tips = [];
    refsBody.refs.forEach(function (r) {
      if (r.commit === true && !seen[r.oid]) {
        seen[r.oid] = true;
        tips.push(r.oid);
      }
    });
    if (refsBody.head !== null && typeof refsBody.head.oid === "string" && !seen[refsBody.head.oid]) {
      tips.push(refsBody.head.oid);
    }
    return tips;
  }

  // 單例輪詢（同 diffPoll／poll：同一時間只有一個分頁是目前分頁）。只偵測變化、不重畫既有內容。
  var graphRefsPoll = { gen: 0, timer: null, controller: null };

  function stopGraphRefsPolling() {
    graphRefsPoll.gen += 1;
    if (graphRefsPoll.timer !== null) {
      clearTimeout(graphRefsPoll.timer);
      graphRefsPoll.timer = null;
    }
    if (graphRefsPoll.controller !== null) {
      graphRefsPoll.controller.abort();
      graphRefsPoll.controller = null;
    }
  }

  function startGraphRefsPolling(tab) {
    stopGraphRefsPolling();
    fetchGraphRefsPoll(tab, graphRefsPoll.gen);
  }

  function fetchGraphRefsPoll(tab, gen) {
    if (gen !== graphRefsPoll.gen || tab.closed) {
      return;
    }
    var controller = new AbortController();
    graphRefsPoll.controller = controller;
    getJson(refsUrl(tab.runtime, tab.rootId), controller.signal).then(function (result) {
      if (gen !== graphRefsPoll.gen) {
        return;
      }
      graphRefsPoll.controller = null;
      if (result.ok && result.body && Array.isArray(result.body.refs) && tab.tips !== null && !tab.refsChanged) {
        var expected = computeExpectedTips(result.body, tab.selectedRefs);
        if (JSON.stringify(expected) !== JSON.stringify(tab.tips)) {
          tab.refsChanged = true;
          renderGraphBanner(tab);
        }
      }
      graphRefsPoll.timer = setTimeout(function () {
        graphRefsPoll.timer = null;
        fetchGraphRefsPoll(tab, gen);
      }, POLL_INTERVAL_MS);
    });
  }

  function captureGraphScroll(tab) {
    tab.scroll.top = tab.els.scroller.scrollTop;
  }

  function restoreGraphScroll(tab) {
    tab.els.scroller.scrollTop = tab.scroll.top;
  }

  // --- 分支篩選 popover（spec「Git Graph 分頁」：可複選、可全選或清除；Esc 關閉、焦點回到按鈕）---

  function rebuildGraphFilterPopover(tab) {
    var body = tab.els.filterBody;
    body.replaceChildren();
    var checkboxes = [];
    if (tab.refsBody !== null) {
      GRAPH_FILTER_GROUPS.forEach(function (g) {
        var items = tab.refsBody.refs.filter(function (r) {
          return r.kind === g.kind;
        });
        if (items.length === 0) {
          return;
        }
        var groupEl = document.createElement("div");
        groupEl.className = "graph-filter-group";
        var titleEl = document.createElement("div");
        titleEl.className = "graph-filter-group-title";
        titleEl.textContent = g.title;
        groupEl.appendChild(titleEl);
        items.forEach(function (r) {
          var label = document.createElement("label");
          label.className = "graph-filter-item";
          var cb = document.createElement("input");
          cb.type = "checkbox";
          cb.value = r.name;
          cb.checked = tab.selectedRefs.indexOf(r.name) !== -1;
          var span = document.createElement("span");
          span.textContent = r.short;
          label.appendChild(cb);
          label.appendChild(span);
          groupEl.appendChild(label);
          checkboxes.push(cb);
        });
        body.appendChild(groupEl);
      });
    }
    tab.els.filterCheckboxes = checkboxes;
  }

  function openGraphFilterPopover(tab) {
    tab.popoverOpen = true;
    tab.els.filterPopover.hidden = false;
    setAttr(tab.els.filterToggle, "aria-expanded", "true");
    var first = tab.els.filterBody.querySelector("input[type=checkbox]");
    if (first !== null) {
      first.focus();
    } else {
      tab.els.filterCancel.focus();
    }
  }

  function closeGraphFilterPopover(tab, refocusButton) {
    tab.popoverOpen = false;
    tab.els.filterPopover.hidden = true;
    setAttr(tab.els.filterToggle, "aria-expanded", "false");
    if (refocusButton) {
      tab.els.filterToggle.focus();
    }
  }

  // --- 搜尋（spec「Git Graph 分頁」：訊息標題／作者名稱／作者 email／hash 前綴，不分大小寫）---

  function normalizeSearchText(s) {
    return typeof s === "string" ? s.toLowerCase() : "";
  }

  function rowMatchesQuery(rowObj, q) {
    return (
      normalizeSearchText(rowObj.subject).indexOf(q) !== -1 ||
      normalizeSearchText(rowObj.author).indexOf(q) !== -1 ||
      normalizeSearchText(rowObj.email).indexOf(q) !== -1 ||
      rowObj.oid.toLowerCase().indexOf(q) === 0
    );
  }

  // 顯示文字（spec「Git Graph 分頁」scenario「搜尋跳轉」：輸入後先只顯示總筆數，按 Enter／
  // Shift+Enter／上下按鈕才跳到第一筆並改成「第 i／共 n 筆」——控制端逐字核對過 scenario：輸入
  // 「FIX」「按兩次 Enter」得到「第 2／共 3 筆」，代表第一次 Enter 落在第 1 筆、第二次才是第 2
  // 筆，也就是輸入當下不能先跳到第 1 筆，否則兩次 Enter 會落在第 3 筆）。
  function renderGraphSearchState(tab) {
    var text = "";
    if (tab.searchMatches.length > 0) {
      text = tab.searchIndex >= 0 ? "第 " + (tab.searchIndex + 1) + "／共 " + tab.searchMatches.length + " 筆" : "共 " + tab.searchMatches.length + " 筆";
    } else if (tab.searchQuery !== "") {
      text = "沒有符合的結果";
    }
    if (tab.els.searchCount.textContent !== text) {
      tab.els.searchCount.textContent = text;
    }
  }

  // `scroll`：只有使用者移到下一筆／上一筆（Enter、Shift+Enter、上下按鈕）才傳 true 把命中列捲入可見
  // 範圍；背景分批載入後重算命中只更新高亮（ui-fixes task 4.5，design D6），不拉動使用者的捲動位置。
  function highlightGraphSearchCurrent(tab, scroll) {
    tab.rows.forEach(function (r) {
      if (r.els.row.classList.contains("is-search-hit")) {
        r.els.row.classList.remove("is-search-hit");
      }
    });
    if (tab.searchIndex >= 0) {
      var rowObj = tab.rows[tab.searchMatches[tab.searchIndex]];
      rowObj.els.row.classList.add("is-search-hit");
      if (scroll) {
        rowObj.els.row.scrollIntoView({ block: "nearest" });
      }
    }
  }

  // `keepCurrent`：分批載入新列後重算比對清單時，儘量停在原本那一筆（不在清單中就退回第 1 筆）。
  function recomputeGraphSearch(tab, keepCurrent) {
    var q = normalizeSearchText(tab.searchQuery);
    var prevOid = tab.searchIndex >= 0 && tab.searchMatches[tab.searchIndex] !== undefined ? tab.rows[tab.searchMatches[tab.searchIndex]].oid : null;
    tab.searchMatches = [];
    if (q !== "") {
      tab.rows.forEach(function (r, i) {
        if (rowMatchesQuery(r, q)) {
          tab.searchMatches.push(i);
        }
      });
    }
    if (keepCurrent && prevOid !== null) {
      // 分批載入新列後重算：儘量停在原本那一筆；找不到（理論上不會發生，session 內只增量
      // append）就退回「未跳轉」狀態，不擅自跳到第 1 筆。
      var idx = -1;
      for (var i = 0; i < tab.searchMatches.length; i += 1) {
        if (tab.rows[tab.searchMatches[i]].oid === prevOid) {
          idx = i;
          break;
        }
      }
      tab.searchIndex = idx;
    } else {
      // 新查詢：只算出總筆數，不自動跳到第一筆（見上方 renderGraphSearchState() 的說明）。
      tab.searchIndex = -1;
    }
    renderGraphSearchState(tab);
    highlightGraphSearchCurrent(tab, false);
  }

  // delta > 0＝Enter／「下一筆」，< 0＝Shift+Enter／「上一筆」。尚未跳轉過（searchIndex === -1）
  // 時，第一次移動落在第一筆（往後找）或最後一筆（往前找），之後才在已有的相符清單內循環。
  function moveGraphSearch(tab, delta) {
    if (tab.searchMatches.length === 0) {
      return;
    }
    if (tab.searchIndex < 0) {
      tab.searchIndex = delta > 0 ? 0 : tab.searchMatches.length - 1;
    } else {
      tab.searchIndex = (tab.searchIndex + delta + tab.searchMatches.length) % tab.searchMatches.length;
    }
    renderGraphSearchState(tab);
    highlightGraphSearchCurrent(tab, true);
  }

  function graphIdentity(fields) {
    return fields.runtime + "\n" + fields.rootId;
  }

  function createGraphTab(fields) {
    var label = "Git Graph · " + fields.rootName;
    var shell = buildTabShell("graph", label);
    if (shell === null) {
      return null;
    }
    shell.els.tab.title = label + "\nruntime：" + fields.runtime;
    setAttr(shell.els.tab, "data-graph-root", fields.rootId);

    var panel = shell.els.panel;
    panel.classList.add("file-panel");

    var toolbar = document.createElement("div");
    toolbar.className = "diff-toolbar"; // 重用既有的可換行工具列（同款排版，不新增選擇器）

    var filterWrap = document.createElement("div");
    filterWrap.className = "graph-filter";
    var filterToggle = document.createElement("button");
    filterToggle.type = "button";
    filterToggle.className = "action-button";
    filterToggle.textContent = "分支篩選";
    filterToggle.setAttribute("data-action", "graph-filter-toggle");
    filterToggle.setAttribute("aria-haspopup", "true");
    filterToggle.setAttribute("aria-expanded", "false");
    var filterPopover = document.createElement("div");
    filterPopover.className = "graph-filter-popover";
    filterPopover.hidden = true;
    filterPopover.setAttribute("role", "dialog");
    filterPopover.setAttribute("aria-label", "分支篩選");
    var filterBody = document.createElement("div");
    filterBody.className = "graph-filter-body";
    var filterActions = document.createElement("div");
    filterActions.className = "graph-filter-actions";
    var filterAllBtn = document.createElement("button");
    filterAllBtn.type = "button";
    filterAllBtn.className = "action-button";
    filterAllBtn.textContent = "全選";
    filterAllBtn.setAttribute("data-action", "graph-filter-all");
    var filterNoneBtn = document.createElement("button");
    filterNoneBtn.type = "button";
    filterNoneBtn.className = "action-button";
    filterNoneBtn.textContent = "清除";
    filterNoneBtn.setAttribute("data-action", "graph-filter-none");
    var filterCancel = document.createElement("button");
    filterCancel.type = "button";
    filterCancel.className = "action-button";
    filterCancel.textContent = "取消";
    filterCancel.setAttribute("data-action", "graph-filter-cancel");
    var filterApply = document.createElement("button");
    filterApply.type = "button";
    filterApply.className = "action-button";
    filterApply.textContent = "套用";
    filterApply.setAttribute("data-action", "graph-filter-apply");
    filterActions.appendChild(filterAllBtn);
    filterActions.appendChild(filterNoneBtn);
    filterActions.appendChild(filterCancel);
    filterActions.appendChild(filterApply);
    filterPopover.appendChild(filterBody);
    filterPopover.appendChild(filterActions);
    filterWrap.appendChild(filterToggle);
    filterWrap.appendChild(filterPopover);

    var searchWrap = document.createElement("div");
    searchWrap.className = "graph-search";
    var searchInput = document.createElement("input");
    searchInput.type = "search";
    searchInput.className = "graph-search-input";
    searchInput.placeholder = "搜尋 commit";
    searchInput.setAttribute("aria-label", "搜尋 commit");
    var searchCount = document.createElement("span");
    searchCount.className = "graph-search-count";
    var searchPrev = document.createElement("button");
    searchPrev.type = "button";
    searchPrev.className = "action-button";
    searchPrev.textContent = "上一筆";
    searchPrev.setAttribute("data-action", "graph-search-prev");
    var searchNext = document.createElement("button");
    searchNext.type = "button";
    searchNext.className = "action-button";
    searchNext.textContent = "下一筆";
    searchNext.setAttribute("data-action", "graph-search-next");
    searchWrap.appendChild(searchInput);
    searchWrap.appendChild(searchCount);
    searchWrap.appendChild(searchPrev);
    searchWrap.appendChild(searchNext);

    var refreshBtn = document.createElement("button");
    refreshBtn.type = "button";
    refreshBtn.className = "action-button files-refresh";
    refreshBtn.textContent = "重新整理";
    refreshBtn.setAttribute("data-action", "graph-refresh");

    toolbar.appendChild(filterWrap);
    toolbar.appendChild(searchWrap);
    toolbar.appendChild(refreshBtn);

    var banner = document.createElement("div");
    banner.className = "action-banner"; // 重用既有的提示列樣式（改綁提示、409 錯誤提示同款）
    banner.hidden = true;
    var bannerText = document.createElement("span");
    bannerText.className = "action-banner-text";
    bannerText.textContent = "分支已變更";
    var bannerReload = document.createElement("button");
    bannerReload.type = "button";
    bannerReload.className = "action-button";
    bannerReload.textContent = "重新載入";
    bannerReload.setAttribute("data-action", "graph-reload");
    banner.appendChild(bannerText);
    banner.appendChild(bannerReload);

    var status = document.createElement("p");
    status.className = "file-status";
    status.hidden = true;

    // 複製回饋（git-review task 4.5；spec「commit 詳情與比較」：複製使用瀏覽器剪貼簿，成功或失敗都
    // 以文字提示回饋；控制端裁決：aria-live="polite" 區域，2 秒後消失）。獨立於詳情面板之外常駐
    // （不隨 renderGraphDetail() 重建），這樣 2 秒計時器不會被詳情重建打斷。
    var copyFeedback = document.createElement("p");
    copyFeedback.className = "file-status";
    copyFeedback.setAttribute("aria-live", "polite");
    copyFeedback.hidden = true;

    var scroller = document.createElement("div");
    scroller.className = "file-viewer-host graph-scroll";

    var listbox = document.createElement("div");
    listbox.className = "graph-listbox";
    listbox.setAttribute("role", "listbox");
    listbox.setAttribute("aria-label", "Commit 清單");

    var loadMoreBtn = document.createElement("button");
    loadMoreBtn.type = "button";
    loadMoreBtn.className = "action-button graph-load-more";
    loadMoreBtn.textContent = "載入更多";
    loadMoreBtn.hidden = true;
    loadMoreBtn.setAttribute("data-action", "graph-load-more");

    var capNote = document.createElement("p");
    capNote.className = "tree-note"; // 重用（不新增只有這裡才用得到的選擇器）
    capNote.textContent = "已達上限 5000 筆，可用分支篩選縮小範圍";
    capNote.hidden = true;

    var emptyNote = document.createElement("p");
    emptyNote.className = "tree-note";
    emptyNote.textContent = "沒有可顯示的 commit";
    emptyNote.hidden = true;

    scroller.appendChild(listbox);
    scroller.appendChild(loadMoreBtn);
    scroller.appendChild(capNote);
    scroller.appendChild(emptyNote);

    panel.appendChild(toolbar);
    panel.appendChild(banner);
    panel.appendChild(status);
    panel.appendChild(copyFeedback);
    panel.appendChild(scroller);

    var tab = {
      runtime: fields.runtime,
      rootId: fields.rootId,
      rootName: fields.rootName,
      rows: [],
      rowByOid: new Map(),
      tips: null,
      hasMore: false,
      atCap: false,
      maxCol: 0,
      trackWidth: GRAPH_COL_W,
      loadStatus: "idle",
      loadCode: null,
      loadGen: 0,
      loadController: null,
      refsBody: null,
      selectedRefs: [],
      refsChanged: false,
      selectedOid: null,
      focusedIndex: -1,
      searchQuery: "",
      searchMatches: [],
      searchIndex: -1,
      scroll: { top: 0 },
      popoverOpen: false,
      // git-review task 4.5：commit 詳情／比較狀態（spec「commit 詳情與比較」）。
      compareBaseOid: null, // 已選為比較基準的 commit oid，或 null
      compareMode: "direct", // "direct" | "fork"（僅在 detailKind === "compare" 時有意義）
      detailKind: null, // null | "single" | "compare"
      detailAnchorOid: null, // 詳情面板目前緊接在哪個 oid 的列後面，或 null（收合）
      detailData: null, // single：commit 端點回應；compare：{baseOid, targetOid, effectiveBase, files, truncated}
      detailStatus: "idle", // idle|loading|ok|error
      detailCode: null,
      detailReqGen: 0,
      detailFocusLost: null, // ui-fixes task 4.8：重建時找不到對應元素、焦點退到容器的那個身分，等下次重建再找
      closed: false,
      els: {
        wrap: shell.els.wrap,
        tab: shell.els.tab,
        close: shell.els.close,
        panel: panel,
        filterToggle: filterToggle,
        filterPopover: filterPopover,
        filterBody: filterBody,
        filterAllBtn: filterAllBtn,
        filterNoneBtn: filterNoneBtn,
        filterCancel: filterCancel,
        filterApply: filterApply,
        filterCheckboxes: [],
        searchInput: searchInput,
        searchCount: searchCount,
        searchPrev: searchPrev,
        searchNext: searchNext,
        refreshBtn: refreshBtn,
        banner: banner,
        bannerReload: bannerReload,
        status: status,
        copyFeedback: copyFeedback,
        scroller: scroller,
        listbox: listbox,
        loadMoreBtn: loadMoreBtn,
        capNote: capNote,
        emptyNote: emptyNote,
        detailWrap: null,
      },
    };

    // Esc 取消比較基準（spec「commit 詳情與比較」）：掛在整個分頁內容區（不是 listbox 本身），因為
    // 詳情面板裡的按鈕（複製、比較切換）不是 `.graph-row`，聚焦在那些按鈕上按 Esc 也該生效；跟
    // 分支篩選 popover 自己的 Esc handler（關 popover）互不干擾——popover 開著時兩者都可能觸發，
    // 各自只管自己的狀態，不衝突。
    panel.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && tab.compareBaseOid !== null) {
        event.preventDefault();
        cancelGraphCompareBase(tab);
      }
    });

    filterToggle.addEventListener("click", function () {
      if (tab.popoverOpen) {
        closeGraphFilterPopover(tab, false);
      } else {
        rebuildGraphFilterPopover(tab);
        openGraphFilterPopover(tab);
      }
    });
    filterPopover.addEventListener("keydown", function (event) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeGraphFilterPopover(tab, true);
      }
    });
    filterAllBtn.addEventListener("click", function () {
      tab.els.filterCheckboxes.forEach(function (cb) {
        cb.checked = true;
      });
    });
    filterNoneBtn.addEventListener("click", function () {
      tab.els.filterCheckboxes.forEach(function (cb) {
        cb.checked = false;
      });
    });
    filterCancel.addEventListener("click", function () {
      closeGraphFilterPopover(tab, true);
    });
    filterApply.addEventListener("click", function () {
      var selected = tab.els.filterCheckboxes
        .filter(function (cb) {
          return cb.checked;
        })
        .map(function (cb) {
          return cb.value;
        });
      tab.selectedRefs = selected;
      closeGraphFilterPopover(tab, true);
      loadInitialGraph(tab);
    });
    searchInput.addEventListener("input", function () {
      tab.searchQuery = searchInput.value;
      recomputeGraphSearch(tab, false);
    });
    searchInput.addEventListener("keydown", function (event) {
      if (event.key === "Enter") {
        event.preventDefault();
        moveGraphSearch(tab, event.shiftKey ? -1 : 1);
      }
    });
    searchPrev.addEventListener("click", function () {
      moveGraphSearch(tab, -1);
    });
    searchNext.addEventListener("click", function () {
      moveGraphSearch(tab, 1);
    });
    refreshBtn.addEventListener("click", function () {
      loadInitialGraph(tab);
    });
    bannerReload.addEventListener("click", function () {
      loadInitialGraph(tab);
    });
    scroller.addEventListener("scroll", function () {
      if (!panel.hidden) {
        tab.scroll.top = scroller.scrollTop;
      }
      maybeAutoLoadMore(tab);
    });
    loadMoreBtn.addEventListener("click", function () {
      if (tab.hasMore && tab.loadStatus !== "loading") {
        loadGraphBatch(tab, tab.rows.length);
      }
    });
    listbox.addEventListener("keydown", function (event) {
      onGraphListKeydown(tab, event);
    });
    listbox.addEventListener("focusin", function (event) {
      var row = event.target instanceof Element ? event.target.closest(".graph-row") : null;
      if (row === null) {
        return;
      }
      var idx = graphRowElements(tab).indexOf(row);
      if (idx >= 0) {
        tab.focusedIndex = idx;
        applyRovingTabindexGraph(tab);
      }
    });

    loadInitialGraph(tab);
    return tab;
  }

  var GRAPH_KIND = {
    identity: graphIdentity,
    create: createGraphTab,
    // 成為目前分頁：捲動位置寫回、開始 refs 輪詢（內部一定先 stopGraphRefsPolling()）。
    activate: function (tab) {
      restoreGraphScroll(tab);
      startGraphRefsPolling(tab);
      maybeAutoLoadMore(tab);
    },
    // 被切走：記下捲動位置、停止 refs 輪詢（同 diff／file kind 的既有裁決：自己在這裡停自己的
    // 輪詢，不依賴「entering 是什麼」）。
    deactivate: function (tab) {
      captureGraphScroll(tab);
      stopGraphRefsPolling();
    },
    // 分頁被關閉：世代加一（之後才回來的分批讀取直接丟棄），中止進行中的讀取（輪詢由
    // deactivate() 在切走時已經停過，關閉分頁前一定會先被切走）。
    dispose: function (tab) {
      tab.closed = true;
      tab.loadGen += 1;
      if (tab.loadController !== null) {
        tab.loadController.abort();
        tab.loadController = null;
      }
    },
    // 分頁還原（git-review task 4.4；design D9 的 v:2 格式）：內容（commit 清單、篩選、搜尋、
    // 選取）不存，還原後回到未載入狀態、由 create() 觸發的一次性讀取重新取得（同 diff kind 的
    // 既有裁決：diffBody 不存，還原後重讀）。
    serialize: function (tab) {
      return { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName };
    },
    deserialize: function (obj) {
      if (obj === null || typeof obj !== "object") {
        return null;
      }
      if (!isNonEmptyString(obj.runtime) || !isNonEmptyString(obj.rootId) || typeof obj.rootName !== "string") {
        return null;
      }
      return { runtime: obj.runtime, rootId: obj.rootId, rootName: obj.rootName };
    },
  };

  // ---------------------------------------------------------------------------
  // 某版本檔案分頁（git-review task 4.5；design D9；spec git-review「某版本檔案分頁」）
  // ---------------------------------------------------------------------------
  //
  // 分頁身分（design D9）：runtime＋root_id＋版本＋路徑；標題（spec）：icon、檔名與「@ 短 hash」／
  // 「@ 暫存區」——task 4.3 已經是這個最終形狀（因為 diff 工具列的「看左側／右側版本」要能以正確
  // 身分開出這個分頁）。task 4.5 補內容：沿用 viewers.js 的四種檢視器（design D9：ctx 由呼叫端提供
  // 「原始內容網址」「渲染網址」「相對連結開啟方式」——查證見下方 revCtx()，viewers.js 本身完全不
  // 認識 git 端點，這幾個網址與 openFile() 全部由這裡組裝，不需要改動 viewers.js 一行）。commit
  // 版本只在第一次成為目前分頁時讀一次、之後不重讀（同 files.js `restoreTabs()` 對還原分頁的既有
  // 裁決：其餘分頁在第一次成為目前分頁時才查）；暫存區（INDEX）版本只在此分頁為目前分頁時輪詢
  // `meta`，`blob`（內容的物件 hash）改變才重讀內容——沿用 diff／graph kind 的單例輪詢裁決（同一時間
  // 只有一個分頁是目前分頁，一個模組變數夠用）。
  //
  // 分頁還原（task 4.5 起支援；4.2／4.3 的「不支援還原」裁決在此撤回——當時內容還是占位文字，還原
  // 一個空分頁沒有意義；現在有真正內容了，理由不再成立）：只存身分（runtime／root_id／rootName／
  // rev／path），跟 diff／graph 一致，內容還原後重新讀取。

  function revIdentity(fields) {
    return fields.runtime + "\n" + fields.rootId + "\n" + fields.rev + "\n" + fields.path;
  }

  function revSideLabel(rev) {
    return rev === "INDEX" ? "暫存區" : shortHash(rev);
  }

  function revNeedsPolling(tab) {
    return tab.rev === "INDEX";
  }

  function revMetaUrl(runtime, rootId, rev, path) {
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/meta/" + seg(rev) + "/" + path.split("/").map(seg).join("/");
  }

  function revBlobUrl(runtime, rootId, rev, path) {
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/blob/" + seg(rev) + "/" + path.split("/").map(seg).join("/");
  }

  function revRenderUrl(runtime, rootId, rev, path) {
    return "/api/git/" + seg(runtime) + "/" + seg(rootId) + "/render/" + seg(rev) + "/" + path.split("/").map(seg).join("/");
  }

  // 內容版本：物件 hash（meta 的 `blob` 欄位）；沒有（理論上不會，防禦用）才退回 size。
  function revContentSig(meta) {
    return typeof meta.blob === "string" && meta.blob !== "" ? meta.blob : "size:" + String(meta.size);
  }

  function renderRevPanel(tab) {
    var els = tab.els;
    if (els.pathEl.textContent !== tab.path) {
      els.pathEl.textContent = tab.path;
    }
    setAttr(els.pathEl, "title", tab.path);
    var versionText = revSideLabel(tab.rev);
    if (els.versionEl.textContent !== versionText) {
      els.versionEl.textContent = versionText;
    }
    var canOpenCurrent = tab.workFile !== null && tab.workFile.exists === true;
    els.openCurrentBtn.disabled = !canOpenCurrent;

    var stale = tab.status === "error" && tab.shown !== null;
    if (els.panel.classList.contains("is-stale") !== stale) {
      els.panel.classList.toggle("is-stale", stale);
    }
    setHidden(els.staleLabel, !stale);

    var text = null;
    var tone = "dim";
    if (tab.status === "error") {
      text = gitErrorText(tab.code);
      tone = "warn";
    } else if (tab.shown === null) {
      text = "正在讀取…";
    }
    setHidden(els.status, text === null);
    if (text !== null && els.status.textContent !== text) {
      els.status.textContent = text;
    }
    setAttr(els.status, "data-tone", tone);
  }

  // 工作區是否有這個檔案（「開啟目前版本」的啟用條件；spec「某版本檔案分頁」）：重用 diff kind 已有的
  // file-review 中繼資料端點查詢方式（filesMetaUrl()），只取「存不存在」，不需要 vscode_uri。
  function refreshRevWorkFile(tab) {
    tab.workGen += 1;
    var gen = tab.workGen;
    getJson(filesMetaUrl(tab.runtime, tab.rootId, tab.path)).then(function (result) {
      if (tab.closed || gen !== tab.workGen) {
        return;
      }
      tab.workFile = { exists: result.ok && result.body !== null && typeof result.body === "object" };
      renderRevPanel(tab);
    });
  }

  function applyRevPendingAnchor(tab) {
    var anchors = window.cockpitMarkdownAnchor;
    if (tab.pendingAnchor === null || tab.els.panel.hidden || !anchors || typeof anchors.reveal !== "function") {
      return false;
    }
    if (anchors.reveal(tab.els.host, tab.pendingAnchor)) {
      tab.pendingAnchor = null;
      return true;
    }
    return false;
  }

  function finishRevRead(tab, sig, result) {
    tab.viewing = false;
    tab.readingSig = null;
    if (result.ok) {
      tab.shown = sig;
      tab.status = "ok";
      tab.code = null;
    } else {
      tab.status = "error";
      tab.code = typeof result.code === "string" ? result.code : "unknown";
    }
    renderRevPanel(tab);
    applyRevPendingAnchor(tab);
  }

  // ctx（design D9：由呼叫端提供網址與相對連結開啟方式；viewers.js 完全不知道這是 git 端點）：
  // Markdown 相對連結開啟同一版本的某版本檔案分頁（openFile → openRevFile()）；圖片讀取同一版本的
  // 內容（rawUrlOf → 同一個 rev 的 blob 網址）；HTML iframe 的 src 是 blob 網址本身，相對子資源
  // （同目錄的 css／img）自然落在同一個 rev（design D6：blob／render／meta 把 `<rev>/<相對路徑>`
  // 放在網址路徑，跟 5a 原始內容端點同理）。
  function revCtx(tab, gen) {
    return {
      host: tab.els.host,
      file: { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, path: tab.path },
      meta: tab.meta,
      rawUrl: revBlobUrl(tab.runtime, tab.rootId, tab.rev, tab.path),
      renderUrl: revRenderUrl(tab.runtime, tab.rootId, tab.rev, tab.path),
      rawUrlOf: function (path) {
        return revBlobUrl(tab.runtime, tab.rootId, tab.rev, path);
      },
      openFile: function (path, anchor) {
        openRevFile(tab, path, anchor);
      },
      isCurrent: function () {
        return !tab.closed && tab.gen === gen;
      },
    };
  }

  function startRevRead(tab, sig) {
    tab.gen += 1;
    var gen = tab.gen;
    var viewers = window.cockpitViewers || {};
    var viewer = viewers[tab.meta.viewer];
    if (typeof viewer !== "function") {
      var placeholder = document.createElement("p");
      placeholder.className = "file-placeholder";
      placeholder.textContent = "尚未實作「" + tab.meta.viewer + "」檢視器";
      tab.els.host.replaceChildren(placeholder);
      finishRevRead(tab, sig, { ok: true });
      return;
    }
    var ctx = revCtx(tab, gen);
    tab.viewing = true;
    tab.readingSig = sig;
    Promise.resolve()
      .then(function () {
        return viewer(ctx);
      })
      .then(
        function (result) {
          return result && result.ok === false ? result : { ok: true };
        },
        function () {
          return { ok: false, code: "unknown" };
        }
      )
      .then(function (result) {
        if (tab.closed || tab.gen !== gen) {
          return;
        }
        finishRevRead(tab, sig, result);
      });
  }

  function applyRevMeta(tab, result) {
    var body = result.ok ? result.body : null;
    if (body === null || typeof body !== "object" || typeof body.viewer !== "string") {
      tab.status = "error";
      tab.code = result.ok ? "unknown" : result.code;
      renderRevPanel(tab);
      return;
    }
    tab.meta = body;
    var iconName = typeof body.icon === "string" && body.icon !== "" ? body.icon : DEFAULT_FILE_ICON;
    if (tab.els.icon !== null) {
      setAttr(tab.els.icon, "src", ICON_BASE + seg(iconName));
    }
    var sig = revContentSig(body);
    if (sig !== (tab.viewing ? tab.readingSig : tab.shown)) {
      if (tab.status !== "error") {
        tab.status = "ok";
      }
      startRevRead(tab, sig);
    } else if (!tab.viewing) {
      tab.status = "ok";
      tab.code = null;
    }
    renderRevPanel(tab);
  }

  // commit 版本：第一次成為目前分頁時讀一次（見 REV_KIND.activate()），之後不再重讀（spec「commit
  // 版本不重複讀取」）。
  function fetchRevOnce(tab) {
    var controller = new AbortController();
    tab.metaController = controller;
    if (tab.status === "idle") {
      tab.status = "loading";
      renderRevPanel(tab);
    }
    getJson(revMetaUrl(tab.runtime, tab.rootId, tab.rev, tab.path), controller.signal).then(function (result) {
      if (tab.closed) {
        return;
      }
      tab.metaController = null;
      applyRevMeta(tab, result);
    });
    refreshRevWorkFile(tab);
  }

  // 暫存區（INDEX）版本：只在此分頁為目前分頁時輪詢（單例，同 diffPoll／graphRefsPoll：同一時間只有
  // 一個分頁是目前分頁）。
  var revPoll = { gen: 0, timer: null, controller: null };

  function stopRevPolling() {
    revPoll.gen += 1;
    if (revPoll.timer !== null) {
      clearTimeout(revPoll.timer);
      revPoll.timer = null;
    }
    if (revPoll.controller !== null) {
      revPoll.controller.abort();
      revPoll.controller = null;
    }
  }

  function startRevPolling(tab) {
    stopRevPolling();
    fetchRevPoll(tab, revPoll.gen);
  }

  function fetchRevPoll(tab, gen) {
    if (gen !== revPoll.gen || tab.closed) {
      return;
    }
    var controller = new AbortController();
    revPoll.controller = controller;
    getJson(revMetaUrl(tab.runtime, tab.rootId, tab.rev, tab.path), controller.signal).then(function (result) {
      if (gen !== revPoll.gen) {
        return;
      }
      revPoll.controller = null;
      applyRevMeta(tab, result);
      refreshRevWorkFile(tab);
      revPoll.timer = setTimeout(function () {
        revPoll.timer = null;
        fetchRevPoll(tab, gen);
      }, POLL_INTERVAL_MS);
    });
  }

  function captureRevScroll(tab) {
    tab.scroll.top = tab.els.host.scrollTop;
  }

  function restoreRevScroll(tab) {
    tab.els.host.scrollTop = tab.scroll.top;
  }

  // Markdown 相對連結（同一版本的另一個檔案）：開同一個 rev 的某版本分頁，帶錨點時等內容畫好才捲
  // （同 files.js openFile() 的既有做法）。
  function openRevFile(tab, path, anchor) {
    if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
      return;
    }
    var opened = window.cockpitFiles.openTab("rev", { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, rev: tab.rev, path: path });
    if (opened === null || typeof anchor !== "string" || anchor === "") {
      return;
    }
    opened.pendingAnchor = anchor;
    if (!applyRevPendingAnchor(opened) && !opened.viewing && opened.shown !== null) {
      opened.pendingAnchor = null; // 內容已畫好卻找不到該標題：不留到之後的重讀才突然捲動
    }
  }

  function isRevToken(value) {
    return value === "INDEX" || /^[0-9a-f]{40}$/.test(value) || /^[0-9a-f]{64}$/.test(value);
  }

  function createRevTab(fields) {
    var name = baseName(fields.path);
    var iconName = typeof fields.icon === "string" && fields.icon !== "" ? fields.icon : DEFAULT_FILE_ICON;
    var label = name + " @ " + revSideLabel(fields.rev);
    var shell = buildTabShell("rev", label, iconName);
    if (shell === null) {
      return null;
    }
    shell.els.tab.title = fields.path + "\n版本：" + revSideLabel(fields.rev) + "\n根目錄：" + fields.rootName;
    setAttr(shell.els.tab, "data-rev-path", fields.path);
    setAttr(shell.els.tab, "data-rev", fields.rev);

    var panel = shell.els.panel;
    panel.classList.add("file-panel");

    var toolbar = document.createElement("div");
    toolbar.className = "file-toolbar"; // 重用檔案分頁的三項式工具列（路徑／次要中繼資訊／一個按鈕）
    var pathEl = document.createElement("span");
    pathEl.className = "file-toolbar-path";
    var staleLabel = document.createElement("span");
    staleLabel.className = "file-stale-label";
    staleLabel.textContent = "過期";
    staleLabel.hidden = true;
    var versionEl = document.createElement("span");
    versionEl.className = "diff-toolbar-versions"; // 重用（等寬次要文字），同款版本標示（同 diff 工具列）
    var openCurrentBtn = document.createElement("button");
    openCurrentBtn.type = "button";
    openCurrentBtn.className = "action-button";
    openCurrentBtn.textContent = "開啟目前版本";
    toolbar.appendChild(pathEl);
    toolbar.appendChild(staleLabel);
    toolbar.appendChild(versionEl);
    toolbar.appendChild(openCurrentBtn);

    var status = document.createElement("p");
    status.className = "file-status";
    status.hidden = true;

    var host = document.createElement("div");
    host.className = "file-viewer-host";

    panel.appendChild(toolbar);
    panel.appendChild(status);
    panel.appendChild(host);

    var tab = {
      runtime: fields.runtime,
      rootId: fields.rootId,
      rootName: fields.rootName,
      rev: fields.rev,
      path: fields.path,
      meta: null,
      status: "idle",
      code: null,
      gen: 0,
      shown: null,
      readingSig: null,
      viewing: false,
      readOnce: false,
      workFile: null,
      workGen: 0,
      metaController: null,
      pendingAnchor: null,
      scroll: { top: 0 },
      closed: false,
      els: {
        wrap: shell.els.wrap,
        tab: shell.els.tab,
        close: shell.els.close,
        panel: panel,
        icon: shell.els.icon,
        pathEl: pathEl,
        staleLabel: staleLabel,
        versionEl: versionEl,
        openCurrentBtn: openCurrentBtn,
        status: status,
        host: host,
      },
    };

    host.addEventListener("scroll", function () {
      if (!panel.hidden) {
        tab.scroll.top = host.scrollTop;
      }
    });
    openCurrentBtn.addEventListener("click", function () {
      if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
        return;
      }
      window.cockpitFiles.openTab("file", { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, path: tab.path });
    });

    renderRevPanel(tab);
    return tab;
  }

  var REV_KIND = {
    identity: revIdentity,
    create: createRevTab,
    // 成為目前分頁：捲動位置寫回；INDEX 版本開始輪詢（內部一定先 stopRevPolling()）；commit 版本
    // 只在「第一次」成為目前分頁時讀一次（spec「commit 版本不重複讀取」；還原的分頁同 file kind 的
    // 既有裁決：其餘分頁在第一次成為目前分頁時才查）。
    activate: function (tab) {
      restoreRevScroll(tab);
      if (revNeedsPolling(tab)) {
        startRevPolling(tab);
      } else if (!tab.readOnce) {
        tab.readOnce = true;
        fetchRevOnce(tab);
      }
    },
    // 被切走：記下捲動位置；INDEX 版本停止輪詢（不管接下來要換去哪個 kind，同 diff／graph kind 的
    // 既有裁決）；commit 版本的一次性讀取不必在切走時中止（同 diff kind 的一次性讀取，dispose() 才
    // abort）。
    deactivate: function (tab) {
      captureRevScroll(tab);
      if (revNeedsPolling(tab)) {
        stopRevPolling();
      }
    },
    // 分頁被關閉：世代加一、中止進行中的讀取、釋放檢視器資源（PDF 的文件、worker、render task；同
    // file kind 的 dispose()）。
    dispose: function (tab) {
      tab.closed = true;
      tab.gen += 1;
      if (tab.metaController !== null) {
        tab.metaController.abort();
        tab.metaController = null;
      }
      var viewerHost = window.cockpitViewerHost;
      if (viewerHost && typeof viewerHost.release === "function") {
        viewerHost.release(tab.els.host);
      }
    },
    // 分頁還原（git-review task 4.5 起支援；4.2／4.3 階段內容還是占位文字，還原沒有意義，這裡撤回
    // 那個裁決）：只存身分，內容還原後重新讀取（同 diff／graph kind）。
    serialize: function (tab) {
      return { runtime: tab.runtime, rootId: tab.rootId, rootName: tab.rootName, rev: tab.rev, path: tab.path };
    },
    deserialize: function (obj) {
      if (obj === null || typeof obj !== "object") {
        return null;
      }
      if (!isNonEmptyString(obj.runtime) || !isNonEmptyString(obj.rootId) || typeof obj.rootName !== "string") {
        return null;
      }
      if (!isRevToken(obj.rev)) {
        return null;
      }
      if (!isRelPathLike(obj.path)) {
        return null;
      }
      return { runtime: obj.runtime, rootId: obj.rootId, rootName: obj.rootName, rev: obj.rev, path: obj.path };
    },
  };

  window.cockpitGit = {
    kinds: {
      diff: DIFF_KIND,
      graph: GRAPH_KIND,
      rev: REV_KIND,
    },
  };

  // ---------------------------------------------------------------------------
  // 左欄「變更」面板（spec git-review「左欄變更分頁」；design D9）
  // ---------------------------------------------------------------------------

  var changesPanelEl = document.getElementById("changes-panel");
  var changesEmptyEl = changesPanelEl ? changesPanelEl.querySelector(".files-empty") : null;

  var changesHead = null;
  var changesRootNameEl = null;
  var changesRuntimeEl = null;
  var changesBranchEl = null;
  var changesStaleLabelEl = null;
  var graphButton = null;
  var refreshButton = null;
  var changesStatusEl = null;
  var changesListEl = null;

  function buildChangesSkeleton() {
    if (changesPanelEl === null) {
      return;
    }
    changesHead = document.createElement("div");
    changesHead.className = "files-head";
    changesHead.hidden = true;
    var names = document.createElement("div");
    names.className = "files-head-names";
    changesRootNameEl = document.createElement("span");
    changesRootNameEl.className = "files-root-name";
    changesRuntimeEl = document.createElement("span");
    changesRuntimeEl.className = "files-runtime";
    // 分支資訊（design 控制端裁決：跟 runtime id 同一款次要中繼資訊樣式，重用 .files-runtime，不為
    // 這一個位置多開一條選擇器）。
    changesBranchEl = document.createElement("span");
    changesBranchEl.className = "files-runtime";
    changesBranchEl.hidden = true;
    changesStaleLabelEl = document.createElement("span");
    changesStaleLabelEl.className = "file-stale-label";
    changesStaleLabelEl.textContent = "過期";
    changesStaleLabelEl.hidden = true;
    names.appendChild(changesRootNameEl);
    names.appendChild(changesRuntimeEl);
    names.appendChild(changesBranchEl);
    names.appendChild(changesStaleLabelEl);
    changesHead.appendChild(names);

    // fix round 1（控制端設計審核）：按鈕獨立一行（見 style.css `#changes-panel .files-head` 的
    // 註解）——「變更」面板的頭部比「檔案」面板多了分支資訊與第二顆按鈕，同一行放不下時（design D2
    // 三欄版面左欄的固定寬度）之前會被擠到跟名稱重疊；`.files-head` 本身維持給「檔案」面板用的單行
    // 版面不變，只在 `#changes-panel` 範圍內覆寫成兩行。
    var actions = document.createElement("div");
    actions.className = "changes-head-actions";

    graphButton = document.createElement("button");
    graphButton.type = "button";
    graphButton.className = "action-button files-refresh";
    graphButton.textContent = "Git Graph";
    graphButton.hidden = true;
    // 約定（同 actions.js 既有的 data-action 慣例）：純測試掛鉤，跟「重新整理」共用 class 沒有其他
    // 可靠的 CSS 選擇器可以只選到這一顆按鈕。
    graphButton.setAttribute("data-action", "open-git-graph");
    graphButton.addEventListener("click", onGraphButtonClick);
    actions.appendChild(graphButton);

    refreshButton = document.createElement("button");
    refreshButton.type = "button";
    refreshButton.className = "action-button files-refresh";
    refreshButton.textContent = "重新整理";
    refreshButton.addEventListener("click", function () {
      if (selectedPane !== null) {
        lookupRoot();
      }
    });
    actions.appendChild(refreshButton);
    changesHead.appendChild(actions);
    changesPanelEl.appendChild(changesHead);

    changesStatusEl = document.createElement("p");
    changesStatusEl.className = "files-status";
    changesStatusEl.hidden = true;
    changesPanelEl.appendChild(changesStatusEl);

    changesListEl = document.createElement("div");
    changesListEl.className = "files-tree";
    changesListEl.setAttribute("role", "list");
    changesListEl.setAttribute("aria-label", "變更清單");
    changesListEl.hidden = true;
    changesListEl.addEventListener("scroll", function () {
      if (!changesListEl.hidden && !changesPanelEl.hidden) {
        changesScrollTop = changesListEl.scrollTop;
      }
    });
    changesPanelEl.appendChild(changesListEl);
  }

  buildChangesSkeleton();

  // --- 模組狀態（同檔頭「變更面板」說明）---

  var selectedPane = null; // { runtime, paneId } | null
  var paneCwds = new Map();
  var view = null; // null | { runtime, paneId, status, code, root, cwd }
  var viewGen = 0;
  var shownRootKey = null; // "runtime\nrootId"：目前畫的清單屬於哪個根目錄
  var changeRowEls = new Map(); // 列鍵 → DOM 節點（只屬於 shownRootKey）
  var gitStatus = { status: "idle", code: null, body: null };
  var poll = { gen: 0, timer: null, controller: null };
  var changesScrollTop = 0;

  function rootKey(runtime, rootId) {
    return runtime + "\n" + rootId;
  }

  function isChangesActive() {
    return !!(window.cockpitFiles && typeof window.cockpitFiles.leftTab === "function" && window.cockpitFiles.leftTab() === "changes");
  }

  function selectedCwd() {
    return selectedPane === null ? undefined : paneCwds.get(selectedPane.runtime + "\n" + selectedPane.paneId);
  }

  function stopPolling() {
    poll.gen += 1;
    if (poll.timer !== null) {
      clearTimeout(poll.timer);
      poll.timer = null;
    }
    if (poll.controller !== null) {
      poll.controller.abort();
      poll.controller = null;
    }
  }

  function startPolling() {
    stopPolling();
    fetchStatus(poll.gen);
  }

  function fetchStatus(gen) {
    if (gen !== poll.gen || !isChangesActive() || view === null || view.root === null || view.root.is_git !== true) {
      return;
    }
    var controller = new AbortController();
    poll.controller = controller;
    var runtime = view.runtime;
    var rootId = view.root.root_id;
    getJson(statusUrl(runtime, rootId), controller.signal).then(function (result) {
      if (gen !== poll.gen) {
        return; // 切換／關閉之後才回來（含被 stopPolling() 中止的那一筆）：丟棄
      }
      poll.controller = null;
      applyStatus(result);
      poll.timer = setTimeout(function () {
        poll.timer = null;
        fetchStatus(gen);
      }, POLL_INTERVAL_MS);
    });
  }

  function applyStatus(result) {
    if (result.ok && result.body && result.body.branch && Array.isArray(result.body.entries)) {
      gitStatus = { status: "ok", code: null, body: result.body };
    } else {
      // 失敗：保留上一次成功的 body（過期呈現），不清空清單。
      gitStatus = { status: "error", code: result.ok ? "unknown" : result.code, body: gitStatus.body };
    }
    renderChangesPanel();
  }

  // 換到另一個根目錄：清掉舊清單與 git 狀態（這個面板不需要 files.js 檔案樹那種「跨根目錄保留狀態」，
  // spec 沒有這個要求），捲動位置歸零。同一個根目錄（key 不變）時什麼都不做，保留目前清單與捲動位置。
  function switchShownRoot(runtime, rootId) {
    var key = rootKey(runtime, rootId);
    if (key === shownRootKey) {
      return;
    }
    shownRootKey = key;
    changeRowEls = new Map();
    if (changesListEl !== null) {
      changesListEl.textContent = "";
    }
    gitStatus = { status: "idle", code: null, body: null };
    changesScrollTop = 0;
  }

  // 查目前選定 pane 的根目錄（同 files.js `lookupRoot()` 的做法，但這個面板沒有「同一 root_id 且 cwd
  // 只是換資料夾」的最佳化必要——git 狀態是整個 repo 的狀態，不因為 pane 的 cwd 換到同一 repo 底下的
  // 子資料夾而需要重讀）。
  function lookupRoot() {
    var pane = selectedPane;
    if (pane === null) {
      view = null;
      renderChangesPanel();
      return;
    }
    var gen = (viewGen += 1);
    var samePane = view !== null && view.runtime === pane.runtime && view.paneId === pane.paneId;
    var previousRoot = samePane ? view.root : null;
    view = { runtime: pane.runtime, paneId: pane.paneId, status: "loading", code: null, root: previousRoot, cwd: selectedCwd() };
    renderChangesPanel();
    getJson(rootUrl(pane.runtime, pane.paneId)).then(function (result) {
      if (gen !== viewGen) {
        return;
      }
      if (!result.ok || typeof result.body.root_id !== "string") {
        view.status = "error";
        view.code = result.ok ? "unknown" : result.code;
        renderChangesPanel();
        return;
      }
      view.status = "ok";
      view.root = result.body;
      switchShownRoot(pane.runtime, result.body.root_id);
      renderChangesPanel();
      if (isChangesActive() && result.body.is_git === true) {
        startPolling();
      }
    });
  }

  // --- 清單重畫（同 files.js `renderTree()` 的鍵比對做法：同一份資料重讀不換掉任何節點）---

  var GROUP_ORDER = [
    { key: "conflict", title: "合併衝突" },
    { key: "staged", title: "已暫存" },
    { key: "unstaged", title: "變更" },
    { key: "untracked", title: "未追蹤" },
  ];

  // 狀態字母的顏色（見檔頭「變更面板」最後一段：行內樣式，理由是避免 CL1 死規則）。
  var STATUS_COLOR = {
    M: "var(--warn)",
    A: "var(--ok)",
    "?": "var(--ok)",
    D: "var(--bad)",
    R: "var(--accent)",
    C: "var(--accent)",
    T: "var(--warn)",
    U: "var(--bad)",
  };

  function desiredChangeRows(body) {
    var byGroup = { conflict: [], staged: [], unstaged: [], untracked: [] };
    body.entries.forEach(function (entry) {
      var list = byGroup[entry.group];
      if (list !== undefined) {
        list.push(entry);
      }
    });
    var rows = [];
    GROUP_ORDER.forEach(function (g) {
      var list = byGroup[g.key];
      if (list.length === 0) {
        return;
      }
      rows.push({ key: "h:" + g.key, kind: "heading", text: g.title + "（" + list.length + "）" });
      list.forEach(function (entry) {
        rows.push({
          key: "r:" + g.key + ":" + entry.path + ":" + (entry.old_path || ""),
          kind: "row",
          group: g.key,
          entry: entry,
        });
      });
    });
    if (body.truncated) {
      rows.push({ key: "n:truncated", kind: "note", text: "變更過多，只列出前面一部分" });
    }
    return rows;
  }

  function createChangeRowEl(desc) {
    if (desc.kind === "heading") {
      var h = document.createElement("div");
      h.className = "changes-group-title";
      return h;
    }
    if (desc.kind === "note") {
      var note = document.createElement("p");
      note.className = "tree-note";
      return note;
    }
    var el = document.createElement("button");
    el.type = "button";
    el.className = "changes-row";
    var icon = document.createElement("img");
    icon.className = "tree-icon";
    icon.alt = "";
    icon.width = 16;
    icon.height = 16;
    icon.draggable = false;
    icon.loading = "lazy";
    var name = document.createElement("span");
    name.className = "tree-name";
    var dir = document.createElement("span");
    dir.className = "changes-dir";
    var status = document.createElement("span");
    status.className = "changes-status";
    el.appendChild(icon);
    el.appendChild(name);
    el.appendChild(dir);
    el.appendChild(status);
    el.addEventListener("click", function (event) {
      onRowActivate(event.currentTarget.__entry, event.currentTarget.__group);
    });
    return el;
  }

  function updateChangeRowEl(el, desc) {
    if (desc.kind !== "row") {
      if (el.textContent !== desc.text) {
        el.textContent = desc.text;
      }
      return;
    }
    var entry = desc.entry;
    el.__entry = entry;
    el.__group = desc.group;
    var icon = el.children[0];
    var nameEl = el.children[1];
    var dirEl = el.children[2];
    var statusEl = el.children[3];
    setAttr(icon, "src", ICON_BASE + seg(entry.icon));
    var renamed = typeof entry.old_path === "string" && entry.old_path !== "";
    var displayName;
    var dirText;
    var titleText;
    if (renamed) {
      displayName = entry.old_path + " → " + entry.path;
      dirText = "";
      titleText = displayName;
    } else {
      displayName = baseName(entry.path);
      dirText = entry.path.length > displayName.length ? entry.path.slice(0, entry.path.length - displayName.length - 1) : "";
      titleText = entry.path;
    }
    if (nameEl.textContent !== displayName) {
      nameEl.textContent = displayName;
    }
    setHidden(dirEl, dirText === "");
    if (dirEl.textContent !== dirText) {
      dirEl.textContent = dirText;
    }
    setAttr(el, "title", titleText);
    setAttr(el, "data-status", entry.status);
    if (statusEl.textContent !== entry.status) {
      statusEl.textContent = entry.status;
    }
    var color = Object.prototype.hasOwnProperty.call(STATUS_COLOR, entry.status) ? STATUS_COLOR[entry.status] : "";
    if (statusEl.style.color !== color) {
      statusEl.style.color = color;
    }
  }

  function renderChangesList() {
    if (changesListEl === null || gitStatus.body === null) {
      return;
    }
    var plan = desiredChangeRows(gitStatus.body);
    var used = new Map();
    var cursor = changesListEl.firstChild;
    plan.forEach(function (desc) {
      var el = changeRowEls.get(desc.key);
      if (el === undefined) {
        el = createChangeRowEl(desc);
      }
      updateChangeRowEl(el, desc);
      used.set(desc.key, el);
      if (el === cursor) {
        cursor = cursor.nextSibling;
      } else {
        changesListEl.insertBefore(el, cursor);
      }
    });
    changeRowEls.forEach(function (el, key) {
      if (!used.has(key) && el.parentNode === changesListEl) {
        changesListEl.removeChild(el);
      }
    });
    changeRowEls = used;
  }

  // --- 開啟 diff／Git Graph 分頁 ---

  // 每組對應的兩側版本（控制端裁決，見 task brief；「合併衝突」見目視驗收 Ruling R11）：
  // 已暫存＝HEAD（沒有 commit 時為空內容）→ 暫存區；變更＝暫存區 → 工作區；合併衝突＝HEAD
  // （沒有 commit 時為空內容，理論上不會發生——要有 commit 才能合併——仍防禦性地沿用「已暫存」
  // 同一套 `headOid || "EMPTY"` 慣例）→ 工作區（未合併的檔案在暫存區有多個版本，無法以暫存區
  // 當左側：`INDEX→WORKTREE` 對這種檔案會得到 `diff --cc` 三方格式，回 409 `unmerged_path`，
  // 見目視驗收缺陷 V1）；未追蹤＝空內容 → 工作區。
  function versionsForGroup(group, headOid) {
    if (group === "staged") {
      return { from: headOid || "EMPTY", to: "INDEX" };
    }
    if (group === "untracked") {
      return { from: "EMPTY", to: "WORKTREE" };
    }
    if (group === "conflict") {
      return { from: headOid || "EMPTY", to: "WORKTREE" };
    }
    return { from: "INDEX", to: "WORKTREE" }; // unstaged
  }

  function onRowActivate(entry, group) {
    if (entry === undefined || view === null || view.root === null) {
      return;
    }
    if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
      return;
    }
    var branch = gitStatus.body !== null ? gitStatus.body.branch : null;
    var v = versionsForGroup(group, branch !== null ? branch.oid : null);
    window.cockpitFiles.openTab("diff", {
      runtime: view.runtime,
      rootId: view.root.root_id,
      rootName: view.root.name,
      from: v.from,
      to: v.to,
      path: entry.path,
      oldPath: entry.old_path || null,
      icon: entry.icon,
    });
  }

  function onGraphButtonClick() {
    if (view === null || view.root === null || view.root.is_git !== true) {
      return;
    }
    if (!window.cockpitFiles || typeof window.cockpitFiles.openTab !== "function") {
      return;
    }
    window.cockpitFiles.openTab("graph", { runtime: view.runtime, rootId: view.root.root_id, rootName: view.root.name });
  }

  // --- 分支資訊文字 ---

  function branchText(branch) {
    var head = branch.head !== null ? branch.head : "分離 HEAD " + shortHash(branch.oid || "");
    var parts = [head];
    if (branch.upstream !== null) {
      var ahead = typeof branch.ahead === "number" ? branch.ahead : 0;
      var behind = typeof branch.behind === "number" ? branch.behind : 0;
      parts.push(branch.upstream + " ↑" + ahead + " ↓" + behind);
    }
    return parts.join(" · ");
  }

  // --- 重畫（進出分頁、輪詢結果、選取改變都呼叫這個）---

  function restoreChangesScroll() {
    if (changesListEl !== null && !changesListEl.hidden && !changesPanelEl.hidden) {
      changesListEl.scrollTop = changesScrollTop;
    }
  }

  function renderChangesPanel() {
    if (changesPanelEl === null) {
      return;
    }
    var sel = selectedPane !== null;
    if (changesEmptyEl !== null) {
      changesEmptyEl.hidden = sel;
    }
    if (changesHead === null) {
      return; // 骨架沒建成（理論上不會發生，#changes-panel 一定存在）
    }
    changesHead.hidden = !sel;
    if (sel) {
      // `view` 可能還是 null：選定 pane 當下若「變更」分頁不是目前分頁，不會立即查根目錄
      // （只有分頁可見時才查，spec「不是目前分頁時不讀取」），這時面板仍要顯示表頭（同 files.js
      // 檔案樹：`selectedPane` 是「有沒有選定」的唯一依據，`view` 只是「查到了什麼」）。
      var rootKnown = view !== null && view.root !== null;
      var name = rootKnown ? view.root.name : selectedPane.paneId;
      if (changesRootNameEl.textContent !== name) {
        changesRootNameEl.textContent = name;
      }
      setAttr(changesRootNameEl, "title", rootKnown ? view.root.root_path : null);
      var runtimeText = view !== null ? view.runtime : selectedPane.runtime;
      if (changesRuntimeEl.textContent !== runtimeText) {
        changesRuntimeEl.textContent = runtimeText;
      }
    }

    var isGit = sel && view !== null && view.status === "ok" && view.root !== null && view.root.is_git === true;
    setHidden(graphButton, !isGit);

    var branchTxt = isGit && gitStatus.body !== null ? branchText(gitStatus.body.branch) : "";
    if (changesBranchEl.textContent !== branchTxt) {
      changesBranchEl.textContent = branchTxt;
    }
    setHidden(changesBranchEl, branchTxt === "");

    var stale = isGit && gitStatus.status === "error" && gitStatus.body !== null;
    // 過期色條：行內樣式（見檔頭「變更面板」最後一段）。
    changesPanelEl.style.boxShadow = stale ? "inset 2px 0 0 0 var(--warn)" : "";
    setHidden(changesStaleLabelEl, !stale);

    var statusText = null;
    var tone = "dim";
    var showList = false;
    if (!sel) {
      // 空狀態已由 changesEmptyEl 顯示。
    } else if (view === null || view.status === "loading") {
      statusText = "正在讀取根目錄…";
    } else if (view.status === "error") {
      statusText = errorText(view.code);
      tone = "warn";
    } else if (view.root.is_git === false) {
      statusText = "這個根目錄不是 git repo";
    } else if (stale) {
      statusText = gitErrorText(gitStatus.code);
      tone = "warn";
      showList = gitStatus.body.entries.length > 0;
    } else if (gitStatus.body === null) {
      statusText = gitStatus.status === "error" ? gitErrorText(gitStatus.code) : "正在讀取變更…";
      tone = gitStatus.status === "error" ? "warn" : "dim";
    } else if (gitStatus.body.entries.length === 0) {
      statusText = "沒有未 commit 的變更";
    } else {
      showList = true;
    }

    setHidden(changesStatusEl, statusText === null);
    if (statusText !== null && changesStatusEl.textContent !== statusText) {
      changesStatusEl.textContent = statusText;
    }
    setAttr(changesStatusEl, "data-tone", tone);

    var wasListHidden = changesListEl.hidden;
    changesListEl.hidden = !showList;
    if (showList) {
      renderChangesList();
      if (wasListHidden) {
        restoreChangesScroll();
      }
    }
  }

  // ---------------------------------------------------------------------------
  // 對外
  // ---------------------------------------------------------------------------

  window.cockpitGit.paneSelected = function (runtime, paneId) {
    var same = selectedPane !== null && selectedPane.runtime === runtime && selectedPane.paneId === paneId;
    selectedPane = { runtime: runtime, paneId: paneId };
    if (!same) {
      viewGen += 1;
      view = null;
      stopPolling();
    }
    if (isChangesActive() && (!same || view === null || view.status === "error")) {
      lookupRoot();
    } else {
      renderChangesPanel();
    }
  };

  window.cockpitGit.paneCleared = function () {
    selectedPane = null;
    viewGen += 1;
    view = null;
    stopPolling();
    renderChangesPanel();
  };

  // 同 files.js `setKnownPanes()`：選定 pane 的 cwd 換到別的根目錄、且「變更」分頁目前可見時重查。
  window.cockpitGit.setKnownPanes = function (panes) {
    var next = new Map();
    for (var i = 0; i < panes.length; i += 1) {
      next.set(panes[i].runtime + "\n" + panes[i].paneId, typeof panes[i].cwd === "string" ? panes[i].cwd : null);
    }
    paneCwds = next;
    if (selectedPane === null || view === null || !isChangesActive()) {
      return;
    }
    var cwd = selectedCwd();
    if (cwd !== undefined && cwd !== view.cwd) {
      lookupRoot();
    }
  };

  // files.js 的 setLeftTab() 切到「變更」分頁時呼叫：立即查一次（還沒查過，或上次失敗）、否則只是
  // 把捲動位置寫回並確保輪詢在跑（同 files.js 檔案分頁 activate() 的時序）。
  window.cockpitGit.changesTabEntered = function () {
    renderChangesPanel();
    restoreChangesScroll();
    if (selectedPane === null) {
      return;
    }
    if (view === null || view.status === "error") {
      lookupRoot();
    } else if (view.root !== null && view.root.is_git === true) {
      startPolling();
    }
  };

  // files.js 的 setLeftTab() 離開「變更」分頁時呼叫：停止輪詢（不是目前分頁時不讀取）。
  window.cockpitGit.changesTabLeft = function () {
    stopPolling();
  };
})();
