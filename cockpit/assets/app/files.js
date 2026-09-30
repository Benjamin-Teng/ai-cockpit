// files.js：左欄「Project／檔案／變更」分頁、檔案樹與中欄下半部分頁區（spec file-review「左欄檔案樹」
// 「檔案分頁」；spec cockpit-dashboard「版面與窄視窗」；design D6）。
//
// file-review task 4.1 只做分頁骨架：
//   - 左欄分頁列（index.html 的 `#files [role="tablist"]`）：目前分頁是本模組的狀態（`leftTab`）。
//     「Project」分頁的內容是 render.js 畫在 `#app` 內的 Project 清單——那些節點每次整頁重畫都會被
//     `replaceChildren` 換掉，從這裡改它的 `hidden` 會在下一次重畫被洗掉，所以改成 render.js 每次
//     重畫時呼叫 `window.cockpitFiles.leftTab()` 自己決定；切換時本模組更新狀態後呼叫既有的重畫入口
//     `window.repaint()`（design D6）。「檔案」分頁的內容（`#files-panel`）在 `#app` 之外，本模組
//     直接切換它的 `hidden`。檔案樹見下方「檔案樹」（file-review task 4.2）。
//   - 下半部分頁列（`#review [role="tablist"]`）：第一個分頁固定是 Live Output、不可關閉；檔案分頁
//     加在它後面（見下方「檔案分頁」）。切換分頁用 `hidden` 屬性（design D6），tab 以 `aria-controls`
//     指向自己的 tabpanel。Live Output 分頁變為不可見／可見時分別經由 output.js 的
//     `window.liveOutput.tabHidden(change)`／`tabShown(change)` 執行切換，讓它停止／恢復輪詢並保住
//     貼底（見 output.js 檔頭「分頁可見性」）。
//   - 以任何方式選定 pane 時切到 Live Output 分頁（spec live-output「選定一個 pane」）：output.js 的
//     select() 呼叫 `window.cockpitFiles.paneSelected(runtime, paneId)`；取消選取（`paneCleared()`）
//     不切換分頁。
//
// git-review task 4.1（design D9）：下半部分頁（Live Output 除外）一般化為帶 `kind` 的物件（目前有內建的
// "file"，以及 `/app/git.js` 未來會提供的 "diff"／"graph"／"rev"），本機儲存升到 v2（每筆帶 `kind`，讀到
// change 之前的 v1 時每筆視為 "file"）。分頁 kind 的完整介面（`identity`／`create`／`activate`／
// `deactivate`／`dispose`／`serialize`／`deserialize`）與框架提供的 `openTab()` 寫在下方「分頁 kind 框架」
// 區塊開頭的註解——4.2–4.5 要掛新 kind 時看那裡，不是這段（這段只講已經固定不變的既有機制）。
//
// 狀態只存在模組變數（memory full-repaint-discards-state-held-only-in-dom）：DOM 只是呈現。
// 兩個分頁列共用同一套鍵盤操作（WAI-ARIA tabs，手動啟用）：方向鍵／Home／End 在分頁間移動焦點，
// Enter／Space 選定（分頁是 `<button>`，由瀏覽器轉成 click）；只有目前分頁 tabindex="0"，Tab 鍵
// 進出分頁列時落在目前分頁上。
//
// 檔案樹（file-review task 4.2；spec file-review「左欄檔案樹」「檔案 icon」；design D6、D10、D11）：
//   - 根目錄來源＝Live Output 的選取（output.js select() → paneSelected()）。只有「檔案」分頁可見時才
//     發請求：切到「檔案」分頁、在「檔案」分頁時選取改變、按「重新整理」時查根目錄
//     （`GET /api/runtimes/<rt>/panes/<pane>/root`），查到後讀根目錄與所有「看得到的」已展開資料夾
//     （`GET /api/files/<rt>/<root_id>/list[/<相對路徑>]`）；資料夾展開時才讀該層。沒有選取時不發任何
//     檔案端點請求（spec「沒有選定 pane」）。在「Project」分頁時選取改變只記下選取，切過來才查。
//   - 同一個 pane 的 cwd 改變也算「根目錄改變」（file-review 最終修正波 F1）：render.js 每次重畫經
//     setKnownPanes() 交來投影中每個 pane 的 cwd，選定 pane 的 cwd 與上次查根目錄時記下的（view.cwd）
//     不同、且左欄在「檔案」分頁時重查一次（在「Project」分頁時不查，切過來本來就會查）。只有 cwd 字串
//     改變才查，不是每份投影都查；查到的 root_id 與畫面上的樹相同時樹不動、也不重讀列目錄。
//   - 狀態只存在模組變數（design D6）：每個根目錄（`runtime + root_id`）一份 RootState——展開的資料夾
//     集合、各資料夾最近一次的列目錄結果、roving tabindex 所在的列、樹的捲動位置。換到別的根目錄再
//     換回來時照這份狀態重建（spec「展開狀態跨根目錄保留」）；取消選取時顯示空狀態，RootState 保留。
//   - 樹是扁平的 `role="treeitem"` 列（`aria-level` 表示深度；files-check 契約 C2），以路徑為鍵逐列
//     比對更新（reconcile）：同一份資料重讀不會換掉任何節點，整頁重畫（render.js，只動 `#app`）本來
//     就碰不到 `#files`，所以重畫期間檔案樹的節點、展開狀態、捲動位置與焦點都不變（spec「重畫不影響
//     檔案樹」）。捲動容器是 tree 本身（契約 C2）。
//   - 錯誤依回應本體的 `code` 查 ERROR_TEXT 顯示中文文案（design D10），不顯示 HTTP 狀態碼或 API 路徑；
//     資料夾讀取失敗只在該資料夾下方顯示原因，不影響樹的其他部分。
//   - icon 以 `<img src="/vendor/material-icons/icons/<檔名>" alt="">` 顯示（design D11；裝飾性）。
//   - 點檔案列（或 Enter）呼叫 `openFile()`（見下方「檔案分頁」）。
//
// 檔案分頁（file-review task 4.3；git-review task 4.1 起是分頁 kind 框架下的 "file" kind，行為不變；
// spec file-review「檔案分頁」「分頁還原」「在 VS Code 開啟」「檔案 icon」；spec cockpit-dashboard「版面與
// 窄視窗」；design D6、D10）：
//   - 狀態是模組變數 `reviewTabs`（依分頁順序，含全部 kind）與 `currentReviewTabId`；檔案分頁以
//     runtime＋root_id＋相對路徑識別，重複開啟切到既有分頁。每個檔案分頁一個 `.review-tab` 包裝元素
//     （role="presentation"），內含
//     分頁本身（`<button role="tab" data-path>`，icon＋檔名，title＝完整相對路徑與根目錄名稱）與關閉按鈕
//     （`aria-label="關閉 <檔名>"`），加在分頁列的 Live Output 之後；內容是 `#review` 內自己的 tabpanel：
//     工具列（相對路徑、最後一次成功讀取的時間、「在 VS Code 開啟」）、狀態列（讀取中／失敗原因）、檢視器
//     容器（`.file-viewer-host`，也是內容的捲動容器；檢視器由 viewers.js 提供）。
//   - 檔案分頁成為目前分頁時立即查中繼資料，之後依自動更新節奏重查（見下方「自動更新」），取得 icon、
//     `vscode_uri`、`viewer`，內容版本（size／modified_ms）與畫面上的不同時依 `viewer` 呼叫 viewers.js
//     重讀（見 viewers.js 檔頭「檢視器入口」）。Live Output 為目前分頁時不查。
//   - 關閉目前分頁 → 右側，沒有右側 → 左側（Live Output 不可關閉）；焦點在檔案分頁上時 Delete 也可關閉。
//   - 切走前記下內容的捲動位置與是否貼底，切回後寫回（design D6）；分頁可見期間捲動位置也隨時記在
//     ft.scroll（file-review task 4.4）。
//   - Markdown 相對連結經由檢視器的 ctx.openFile(path, anchor) 開檔；帶錨點時等內容畫好才捲到該標題
//     （file-review task 4.4；見 openFile()、applyPendingAnchor()）。
//   - 已打開的分頁、目前分頁與左欄目前分頁存在 localStorage（見下方「分頁還原」），載入時還原。
//
// 自動更新（file-review task 4.6；spec file-review「自動更新」「分頁還原」；design D6、D7）：
//   - 只查「目前的檔案分頁」的中繼資料：成為目前分頁時立即查一次；每次查詢結束（成功或失敗）後 2 秒才排
//     下一次（POLL_INTERVAL_MS，setTimeout 串接，不用 setInterval），同一時間至多一個進行中。切換或關閉
//     分頁時 stopPolling()：清掉排好的下一次、abort 進行中的那一筆，並把輪詢世代（poll.gen）加一——之後
//     才回來的回應一律丟棄。Live Output 為目前分頁、或沒有檔案分頁時沒有任何輪詢鏈存在。
//   - 兩種世代分開（控制端裁決）：poll.gen 是「分頁身分」的世代，只在切換／關閉分頁時變；ft.gen 是「內容
//     讀取」的世代（檢視器的 ctx.isCurrent() 以它判斷），只在開始下一次重讀、分頁被切走或關閉時變。輪詢
//     本身不動 ft.gen，所以讀一個大 PDF 的期間照常輪詢不會把這次讀取當成過期丟掉。
//   - 內容版本＝`size:modified_ms`。中繼資料的版本與畫面上顯示的版本（ft.shown；正在讀取時比對正在讀的
//     版本 ft.readingSig）不同時才重讀——同版本不重畫，DOM 節點不換（spec cockpit-dashboard「頻繁重畫不影響
//     檔案分頁」）。重讀由檢視器以 mount() 換內容：markdown／text 保住 host 的 scrollTop（超出新內容長度
//     時瀏覽器取最大值），pdf 沿用舊工作階段的頁碼（超出新總頁數取最後一頁，見 viewers.js「PDF」）。
//   - 查詢或讀取失敗：保留最後一次的內容，畫面上已有內容時標為過期（`.file-panel.is-stale`：內容文字
//     --text-dim、左緣 --warn 色條、工具列「過期」字樣，與 Live Output 相同的呈現；spec live-output「失敗與
//     消失的呈現」），狀態列以 --warn 顯示原因（FILE_ERROR_TEXT／ERROR_TEXT 查表），依原節奏繼續查；重讀
//     成功（或中繼資料顯示畫面上已是最新版本）時過期標示與原因消失。還原的根目錄不可用（root_unavailable）
//     也走這條：沒有內容可保留時只顯示原因，恢復後正常顯示。
//
// 對外：`window.cockpitFiles = { leftTab(), paneSelected(runtime, paneId), paneCleared(), setKnownPanes(panes),
// openTab(kind, fields) }`（沿用 `window.liveOutput`／`window.cockpitActions` 的全域掛勾慣例；`openTab` 是
// git-review task 4.1 新增，見「分頁 kind 框架」）。

(function () {
  "use strict";

  var LEFT_PROJECTS = "projects";
  var LEFT_FILES = "files";
  // git-review task 4.2（design D9）：左欄「變更」分頁——內容（分支資訊、四組變更清單）由 git.js 填入
  // `#changes-panel`；本模組只負責分頁列的通用機制（跟「Project」「檔案」共用同一個 tablist）與
  // `#changes-panel` 的 hidden 切換，進出這個分頁時呼叫 `window.cockpitGit.changesTabEntered()`／
  // `changesTabLeft()` 讓 git.js 自己決定要不要查詢、要不要輪詢（見 setLeftTab()）。
  var LEFT_CHANGES = "changes";
  var LIVE_TAB_ID = "review-tab-live";

  var filesRoot = document.getElementById("files");
  var reviewRoot = document.getElementById("review");

  // --- 模組狀態 ---

  var leftTab = LEFT_PROJECTS;
  var currentReviewTabId = LIVE_TAB_ID;
  var selectedPane = null; // { runtime, paneId } | null（Live Output 的選取＝檔案樹的根目錄來源）

  // --- 分頁列共用 ---

  function tabsOf(tablist) {
    return Array.prototype.slice.call(tablist.querySelectorAll('[role="tab"]'));
  }

  function panelOf(tab) {
    var id = tab.getAttribute("aria-controls");
    return id ? document.getElementById(id) : null;
  }

  // 方向鍵在分頁間移動焦點（不選定）；Enter／Space 由 `<button>` 自己轉成 click。
  function handleTablistKeydown(tablist, event) {
    var tabs = tabsOf(tablist);
    var index = tabs.indexOf(document.activeElement);
    if (index < 0 || tabs.length === 0) {
      return;
    }
    var next = null;
    if (event.key === "ArrowRight") {
      next = tabs[(index + 1) % tabs.length];
    } else if (event.key === "ArrowLeft") {
      next = tabs[(index - 1 + tabs.length) % tabs.length];
    } else if (event.key === "Home") {
      next = tabs[0];
    } else if (event.key === "End") {
      next = tabs[tabs.length - 1];
    }
    if (next !== null) {
      event.preventDefault();
      next.focus();
    }
  }

  function wireTablist(tablist, onActivate) {
    tablist.addEventListener("click", function (event) {
      var tab = event.target instanceof Element ? event.target.closest('[role="tab"]') : null;
      if (tab !== null && tablist.contains(tab)) {
        onActivate(tab);
      }
    });
    tablist.addEventListener("keydown", function (event) {
      handleTablistKeydown(tablist, event);
    });
  }

  function markSelected(tabs, selectedTab) {
    for (var i = 0; i < tabs.length; i += 1) {
      var isSelected = tabs[i] === selectedTab;
      tabs[i].setAttribute("aria-selected", isSelected ? "true" : "false");
      tabs[i].tabIndex = isSelected ? 0 : -1;
    }
  }

  // --- 左欄分頁 ---

  var leftTablist = filesRoot ? filesRoot.querySelector('[role="tablist"]') : null;
  var filesPanel = document.getElementById("files-panel");
  var filesEmpty = filesPanel ? filesPanel.querySelector(".files-empty") : null;
  // git-review task 4.2：內容由 git.js 建立，這裡只切換 hidden（同 filesPanel 的做法）。
  var changesPanel = document.getElementById("changes-panel");

  // --- 檔案樹（file-review task 4.2；見檔頭「檔案樹」）---

  var ICON_BASE = "/vendor/material-icons/icons/";
  var REQUEST_TIMEOUT_MS = 10000;
  var LOADING_ROOT_TEXT = "正在讀取根目錄…";
  var LOADING_DIR_TEXT = "讀取中…";
  var EMPTY_DIR_TEXT = "（空資料夾）";

  // design D10：依錯誤本體的 `code` 查表（spec「檔案端點的共同規則」代碼表），不以 HTTP 狀態碼或 API
  // 路徑開頭。`not_found`／`wrong_kind` 依目標是根目錄查詢還是資料夾給不同說法（見 errorText()）。
  var ERROR_TEXT = {
    forbidden_source: "請求來源不被接受，請從本機的 Cockpit 頁面開啟",
    method_not_allowed: "這個操作不被接受",
    bad_request: "名稱含有無法處理的字元，無法讀取",
    runtime_unknown: "設定中沒有這個 runtime",
    pane_unknown: "這個 pane 已不在目前的畫面中",
    no_root: "這個 pane 沒有可瀏覽的資料夾（沒有回報工作目錄，或該目錄不存在）",
    root_unavailable: "這個資料夾目前無法瀏覽（不在允許的根目錄中）",
    path_outside_root: "這個資料夾指向根目錄以外，不顯示內容",
    not_found: "資料夾已不存在",
    wrong_kind: "這個項目已不是資料夾",
    too_large: "內容太大，無法顯示",
    not_markdown: "不是 Markdown 檔案",
    io_error: "讀取時發生錯誤",
    network: "無法連線到 Cockpit 服務",
    timeout: "讀取逾時（超過 10 秒沒有回應）",
  };
  var UNKNOWN_ERROR_TEXT = "讀取失敗（無法辨識的回應）";

  function errorText(code) {
    return Object.prototype.hasOwnProperty.call(ERROR_TEXT, code) ? ERROR_TEXT[code] : UNKNOWN_ERROR_TEXT;
  }

  function seg(value) {
    return encodeURIComponent(value);
  }

  function rootUrl(runtime, paneId) {
    return "/api/runtimes/" + seg(runtime) + "/panes/" + seg(paneId) + "/root";
  }

  function listUrl(runtime, rootId, path) {
    var url = "/api/files/" + seg(runtime) + "/" + seg(rootId) + "/list";
    if (path !== "") {
      url += "/" + path.split("/").map(seg).join("/");
    }
    return url;
  }

  // 一次 GET：resolve 成 `{ ok: true, body }` 或 `{ ok: false, code }`（不 reject）。`code` 取自錯誤本體；
  // 本體不是 JSON 或沒有 `code` 時為 "unknown"，連線失敗為 "network"，前端逾時為 "timeout"。
  // signal（可省略）：呼叫端自己的 AbortSignal，abort 時一併中止這次請求（自動更新切換分頁時用；file-review
  // task 4.6）；被呼叫端中止的請求一樣 resolve 成 `{ ok: false, code: "network" }`，由呼叫端自己丟棄。
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

  // 每個根目錄一份狀態（design D6：展開狀態依 runtime + root_id）。listings：相對路徑（根目錄為 ""）→
  // `{ entries: Array|null, omitted, skipped, error: code|null, seq }`；seq 讓晚到的舊回應被丟棄。
  var rootStates = new Map();

  function rootKey(runtime, rootId) {
    return runtime + "\n" + rootId;
  }

  function rootStateFor(runtime, rootId) {
    var key = rootKey(runtime, rootId);
    var rs = rootStates.get(key);
    if (rs === undefined) {
      rs = {
        key: key,
        runtime: runtime,
        rootId: rootId,
        expanded: new Set(),
        listings: new Map(),
        focusPath: null,
        scrollTop: 0,
      };
      rootStates.set(key, rs);
    }
    return rs;
  }

  // 目前選取對應的根目錄查詢：null（沒有選取，或在「Project」分頁時選取改變、還沒查）或
  // `{ runtime, paneId, status: "loading"|"ok"|"error", code, root, cwd }`（cwd＝發出查詢當下投影中該 pane
  // 的 cwd，投影裡沒有該 pane 時為 undefined）。viewGen 讓換選取後才到的舊查詢結果被丟棄。
  var view = null;
  var viewGen = 0;
  // 最近一份投影中每個 pane 的 cwd（鍵同 rootKey 的寫法：runtime＋換行＋pane id；值為字串或 null）。
  var paneCwds = new Map();

  function selectedCwd() {
    return selectedPane === null ? undefined : paneCwds.get(selectedPane.runtime + "\n" + selectedPane.paneId);
  }
  // 樹目前畫的是哪個 RootState（查新根目錄期間仍顯示上一棵，查到才換）。
  var shown = null;
  var rowEls = new Map(); // 列的鍵（見 desiredRows()）→ 目前的 DOM 節點（只屬於 shown）

  var treeHead = null;
  var rootNameEl = null;
  var runtimeEl = null;
  var refreshButton = null;
  var treeStatus = null;
  var treeEl = null;

  function buildTreeSkeleton() {
    if (filesPanel === null) {
      return;
    }
    treeHead = document.createElement("div");
    treeHead.className = "files-head";
    treeHead.hidden = true;
    var names = document.createElement("div");
    names.className = "files-head-names";
    rootNameEl = document.createElement("span");
    rootNameEl.className = "files-root-name";
    runtimeEl = document.createElement("span");
    runtimeEl.className = "files-runtime";
    names.appendChild(rootNameEl);
    names.appendChild(runtimeEl);
    treeHead.appendChild(names);
    refreshButton = document.createElement("button");
    refreshButton.type = "button";
    refreshButton.className = "action-button files-refresh";
    refreshButton.textContent = "重新整理";
    refreshButton.addEventListener("click", function () {
      if (selectedPane !== null) {
        lookupRoot();
      }
    });
    treeHead.appendChild(refreshButton);
    filesPanel.appendChild(treeHead);

    treeStatus = document.createElement("p");
    treeStatus.className = "files-status";
    treeStatus.hidden = true;
    filesPanel.appendChild(treeStatus);

    treeEl = document.createElement("div");
    treeEl.className = "files-tree";
    treeEl.setAttribute("role", "tree");
    treeEl.setAttribute("aria-label", "檔案樹");
    treeEl.hidden = true;
    treeEl.addEventListener("click", onTreeClick);
    treeEl.addEventListener("keydown", onTreeKeydown);
    treeEl.addEventListener("focusin", onTreeFocusIn);
    treeEl.addEventListener("scroll", function () {
      if (shown !== null && !treeEl.hidden && !filesPanel.hidden) {
        shown.scrollTop = treeEl.scrollTop;
      }
    });
    filesPanel.appendChild(treeEl);
  }

  // 查目前選取的根目錄；查到後換到該根目錄的樹並重讀根目錄與看得到的已展開資料夾。
  // cwdChanged：由選定 pane 的 cwd 改變觸發（setKnownPanes）。這時若上一次查詢成功、且新的 root_id 與畫面上
  // 的樹相同，樹不動、不重讀（只是在同一個 repo 裡換資料夾）。
  function lookupRoot(cwdChanged) {
    var pane = selectedPane;
    var gen = (viewGen += 1);
    var samePane = view !== null && view.runtime === pane.runtime && view.paneId === pane.paneId;
    var previousRoot = samePane ? view.root : null;
    var keepRootId = cwdChanged === true && samePane && view.status === "ok" && view.root !== null ? view.root.root_id : null;
    view = { runtime: pane.runtime, paneId: pane.paneId, status: "loading", code: null, root: previousRoot, cwd: selectedCwd() };
    renderFilesPanel();
    getJson(rootUrl(pane.runtime, pane.paneId)).then(function (result) {
      if (gen !== viewGen) {
        return;
      }
      if (!result.ok || typeof result.body.root_id !== "string") {
        view.status = "error";
        view.code = result.ok ? "unknown" : result.code;
        renderFilesPanel();
        return;
      }
      view.status = "ok";
      view.root = result.body;
      var rs = rootStateFor(pane.runtime, result.body.root_id);
      if (keepRootId === result.body.root_id && rs === shown) {
        renderFilesPanel(); // cwd 改了但仍在同一個根目錄：樹不動、不重讀
        return;
      }
      // 先發出讀取（listing 因此已登記、renderTree() 不會再補讀同一層），再換樹。
      loadListing(rs, "");
      reachableExpanded(rs).forEach(function (path) {
        loadListing(rs, path);
      });
      showRoot(rs);
      renderFilesPanel();
    });
  }

  function loadListing(rs, path) {
    var listing = rs.listings.get(path);
    if (listing === undefined) {
      listing = { entries: null, omitted: 0, skipped: 0, error: null, seq: 0 };
      rs.listings.set(path, listing);
    }
    listing.seq += 1;
    var seq = listing.seq;
    getJson(listUrl(rs.runtime, rs.rootId, path)).then(function (result) {
      if (listing.seq !== seq) {
        return;
      }
      if (result.ok && Array.isArray(result.body.entries)) {
        listing.entries = result.body.entries;
        listing.omitted = Number(result.body.omitted) || 0;
        listing.skipped = Number(result.body.skipped) || 0;
        listing.error = null;
      } else {
        // 讀取失敗：該資料夾下方只顯示原因（舊的子項目可能已不正確，不再顯示），樹的其他部分不動。
        listing.entries = null;
        listing.error = result.ok ? "unknown" : result.code;
      }
      if (rs === shown) {
        renderTree();
      }
    });
  }

  // 從根目錄沿著「已展開」走得到的資料夾（收合的資料夾底下即使記著展開，也看不到、不讀）。
  function reachableExpanded(rs) {
    var out = [];
    var walk = function (path) {
      var listing = rs.listings.get(path);
      if (listing === undefined || listing.entries === null) {
        return;
      }
      listing.entries.forEach(function (entry) {
        var child = path === "" ? entry.name : path + "/" + entry.name;
        if (entry.kind === "dir" && rs.expanded.has(child)) {
          out.push(child);
          walk(child);
        }
      });
    };
    walk("");
    return out;
  }

  // 目前根目錄應該畫出的列：treeitem（`{ key: "r:<path>", path, name, kind, level, icon, expanded }`）與
  // 提示列（`{ key: "n:<parent>:<kind>", level, text, tone }`，不是 treeitem）。另外回傳看得到、已展開
  // 卻還沒讀過的資料夾，由呼叫端補讀。
  function desiredRows(rs) {
    var rows = [];
    var missing = [];
    var walk = function (path, level) {
      var listing = rs.listings.get(path);
      var noteKey = "n:" + path + ":";
      if (listing === undefined || (listing.entries === null && listing.error === null)) {
        if (listing === undefined) {
          missing.push(path);
        }
        rows.push({ key: noteKey + "loading", level: level, text: path === "" ? LOADING_ROOT_TEXT : LOADING_DIR_TEXT, tone: "dim" });
        return;
      }
      if (listing.error !== null) {
        rows.push({ key: noteKey + "error", level: level, text: errorText(listing.error), tone: "warn" });
        return;
      }
      listing.entries.forEach(function (entry) {
        var child = path === "" ? entry.name : path + "/" + entry.name;
        var isDir = entry.kind === "dir";
        var open = isDir && rs.expanded.has(child);
        rows.push({
          key: "r:" + child,
          path: child,
          name: entry.name,
          kind: isDir ? "dir" : "file",
          level: level,
          icon: open && entry.icon_open ? entry.icon_open : entry.icon,
          expanded: isDir ? open : null,
        });
        if (open) {
          walk(child, level + 1);
        }
      });
      var hiddenCount = listing.omitted + listing.skipped;
      if (hiddenCount > 0) {
        rows.push({ key: noteKey + "more", level: level, text: "還有 " + hiddenCount + " 項未顯示", tone: "dim" });
      } else if (listing.entries.length === 0) {
        rows.push({ key: noteKey + "empty", level: level, text: EMPTY_DIR_TEXT, tone: "dim" });
      }
    };
    walk("", 1);
    return { rows: rows, missing: missing };
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

  function createRowEl(desc) {
    var el = document.createElement("div");
    if (desc.path === undefined) {
      el.className = "tree-note";
      el.textContent = desc.text;
      return el;
    }
    el.className = "tree-row";
    el.setAttribute("role", "treeitem");
    el.tabIndex = -1;
    var twisty = document.createElement("span");
    twisty.className = "tree-twisty";
    twisty.setAttribute("aria-hidden", "true");
    var icon = document.createElement("img");
    icon.className = "tree-icon";
    icon.alt = "";
    icon.width = 16;
    icon.height = 16;
    icon.draggable = false;
    icon.loading = "lazy";
    var name = document.createElement("span");
    name.className = "tree-name";
    name.textContent = desc.name;
    el.appendChild(twisty);
    el.appendChild(icon);
    el.appendChild(name);
    return el;
  }

  // 只在值真的改變時寫 DOM：同一份資料重畫不產生任何 mutation。
  function updateRowEl(el, desc) {
    el.style.setProperty("--tree-level", String(desc.level));
    if (desc.path === undefined) {
      setAttr(el, "data-tone", desc.tone);
      if (el.textContent !== desc.text) {
        el.textContent = desc.text;
      }
      return;
    }
    setAttr(el, "title", desc.path);
    setAttr(el, "data-path", desc.path);
    setAttr(el, "data-kind", desc.kind);
    setAttr(el, "aria-level", String(desc.level));
    setAttr(el, "aria-expanded", desc.expanded === null ? null : String(desc.expanded));
    var icon = el.children[1];
    setAttr(icon, "src", ICON_BASE + seg(desc.icon));
  }

  // 以鍵逐列比對：沿用既有節點、只在順序不對時搬動、刪掉不再需要的列。
  function renderTree() {
    if (treeEl === null || shown === null) {
      return;
    }
    var rs = shown;
    var plan = desiredRows(rs);
    var used = new Map();
    var cursor = treeEl.firstChild;
    plan.rows.forEach(function (desc) {
      var el = rowEls.get(desc.key);
      if (el === undefined) {
        el = createRowEl(desc);
      }
      updateRowEl(el, desc);
      used.set(desc.key, el);
      if (el === cursor) {
        cursor = cursor.nextSibling;
      } else {
        treeEl.insertBefore(el, cursor);
      }
    });
    rowEls.forEach(function (el, key) {
      if (!used.has(key) && el.parentNode === treeEl) {
        treeEl.removeChild(el);
      }
    });
    rowEls = used;
    applyRovingTabindex(rs);
    plan.missing.forEach(function (path) {
      loadListing(rs, path);
    });
  }

  function rowElements() {
    return Array.prototype.filter.call(treeEl.children, function (el) {
      return el.getAttribute("role") === "treeitem";
    });
  }

  // roving tabindex：只有一列 tabindex="0"（上次聚焦的列；它不在了就用第一列），Tab 鍵進出樹時落在它上面。
  function applyRovingTabindex(rs) {
    var rows = rowElements();
    var target = rs.focusPath !== null ? rowEls.get("r:" + rs.focusPath) : undefined;
    if (target === undefined) {
      target = rows.length > 0 ? rows[0] : null;
    }
    rows.forEach(function (row) {
      var want = row === target ? 0 : -1;
      if (row.tabIndex !== want) {
        row.tabIndex = want;
      }
    });
  }

  // 換到另一個根目錄的樹：記下舊樹的捲動位置，清掉舊列，依新根目錄的狀態重建並寫回它的捲動位置。
  function showRoot(rs) {
    if (shown === rs) {
      return;
    }
    if (shown !== null && !treeEl.hidden) {
      shown.scrollTop = treeEl.scrollTop;
    }
    shown = rs;
    rowEls = new Map();
    treeEl.textContent = "";
    renderTree();
  }

  function restoreTreeScroll() {
    if (shown !== null && treeEl !== null && !treeEl.hidden && !filesPanel.hidden) {
      treeEl.scrollTop = shown.scrollTop;
    }
  }

  // 面板上半部（頂端名稱、狀態列）與樹的顯示與否。
  function renderFilesPanel() {
    if (filesEmpty !== null) {
      filesEmpty.hidden = selectedPane !== null;
    }
    if (treeEl === null) {
      return;
    }
    var hasSelection = selectedPane !== null && view !== null;
    var treeVisible = hasSelection && view.status !== "error" && shown !== null && (view.status === "ok" || view.root !== null);
    var wasHidden = treeEl.hidden;
    treeHead.hidden = !hasSelection;
    treeEl.hidden = !treeVisible;
    if (hasSelection) {
      var name = view.root !== null ? view.root.name : view.paneId;
      if (rootNameEl.textContent !== name) {
        rootNameEl.textContent = name;
      }
      setAttr(rootNameEl, "title", view.root !== null ? view.root.root_path : null);
      if (runtimeEl.textContent !== view.runtime) {
        runtimeEl.textContent = view.runtime;
      }
    }
    var status = null;
    var tone = "dim";
    if (hasSelection && view.status === "error") {
      status = errorText(view.code);
      tone = "warn";
    } else if (hasSelection && !treeVisible) {
      status = LOADING_ROOT_TEXT;
    }
    treeStatus.hidden = status === null;
    if (status !== null && treeStatus.textContent !== status) {
      treeStatus.textContent = status;
    }
    setAttr(treeStatus, "data-tone", tone);
    if (wasHidden && treeVisible) {
      restoreTreeScroll();
    }
  }

  function currentRowOf(target) {
    var row = target instanceof Element ? target.closest('[role="treeitem"]') : null;
    return row !== null && row.parentNode === treeEl ? row : null;
  }

  function toggleDir(path) {
    var rs = shown;
    if (rs === null) {
      return;
    }
    if (rs.expanded.has(path)) {
      rs.expanded.delete(path);
      // 焦點在被收起的子孫列上時移回這個資料夾（那一列即將被移除）。
      var active = document.activeElement;
      var activePath = active instanceof Element && active.parentNode === treeEl ? active.getAttribute("data-path") : null;
      if (activePath !== null && activePath.indexOf(path + "/") === 0) {
        rs.focusPath = path;
        var folder = rowEls.get("r:" + path);
        if (folder !== undefined) {
          folder.focus();
        }
      }
      renderTree();
    } else {
      rs.expanded.add(path);
      // spec：資料夾在展開時才讀取該層（每次展開都重讀；先顯示上次讀到的內容）。
      loadListing(rs, path);
      renderTree();
    }
  }

  function activateRow(row) {
    var path = row.getAttribute("data-path");
    if (row.getAttribute("data-kind") === "dir") {
      toggleDir(path);
    } else {
      openFile({
        runtime: shown.runtime,
        rootId: shown.rootId,
        rootName: view !== null && view.root !== null ? view.root.name : "",
        path: path,
      });
    }
  }

  function onTreeClick(event) {
    var row = currentRowOf(event.target);
    if (row !== null) {
      activateRow(row);
    }
  }

  function onTreeFocusIn(event) {
    var row = currentRowOf(event.target);
    if (row !== null && shown !== null) {
      shown.focusPath = row.getAttribute("data-path");
      applyRovingTabindex(shown);
    }
  }

  function focusRow(row) {
    if (row) {
      row.focus();
      row.scrollIntoView({ block: "nearest" });
    }
  }

  // WAI-ARIA tree 的鍵盤操作：上下／Home／End 移動焦點；右鍵展開或進入第一個子項目；左鍵收合或回到
  // 上層資料夾；Enter／Space 切換資料夾，Enter 開啟檔案（spec：Enter／Space 切換展開）。
  function onTreeKeydown(event) {
    var row = currentRowOf(event.target);
    if (row === null || event.altKey || event.ctrlKey || event.metaKey) {
      return;
    }
    var rows = rowElements();
    var index = rows.indexOf(row);
    var path = row.getAttribute("data-path");
    var isDir = row.getAttribute("data-kind") === "dir";
    var expanded = row.getAttribute("aria-expanded") === "true";
    var handled = true;
    if (event.key === "ArrowDown") {
      focusRow(rows[index + 1]);
    } else if (event.key === "ArrowUp") {
      focusRow(rows[index - 1]);
    } else if (event.key === "Home") {
      focusRow(rows[0]);
    } else if (event.key === "End") {
      focusRow(rows[rows.length - 1]);
    } else if (event.key === "ArrowRight") {
      if (isDir && !expanded) {
        toggleDir(path);
      } else if (isDir) {
        var next = rows[index + 1];
        if (next && (next.getAttribute("data-path") || "").indexOf(path + "/") === 0) {
          focusRow(next);
        }
      }
    } else if (event.key === "ArrowLeft") {
      if (isDir && expanded) {
        toggleDir(path);
      } else if (path.indexOf("/") > 0) {
        focusRow(rowEls.get("r:" + path.slice(0, path.lastIndexOf("/"))));
      }
    } else if (event.key === "Enter") {
      activateRow(row);
    } else if (event.key === " ") {
      if (isDir) {
        toggleDir(path);
      }
    } else {
      handled = false;
    }
    if (handled) {
      event.preventDefault();
    }
  }

  buildTreeSkeleton();

  function setLeftTab(id) {
    if (id !== LEFT_PROJECTS && id !== LEFT_FILES && id !== LEFT_CHANGES) {
      return;
    }
    var enteringFiles = id === LEFT_FILES && leftTab !== LEFT_FILES;
    var enteringChanges = id === LEFT_CHANGES && leftTab !== LEFT_CHANGES;
    var leavingChanges = leftTab === LEFT_CHANGES && id !== LEFT_CHANGES;
    leftTab = id;
    if (leftTablist !== null) {
      var tabs = tabsOf(leftTablist);
      var target = null;
      for (var i = 0; i < tabs.length; i += 1) {
        if (tabs[i].getAttribute("data-left-tab") === id) {
          target = tabs[i];
        }
      }
      markSelected(tabs, target);
    }
    if (filesPanel !== null) {
      filesPanel.hidden = id !== LEFT_FILES;
    }
    if (changesPanel !== null) {
      changesPanel.hidden = id !== LEFT_CHANGES;
    }
    // 切到「檔案」分頁：有選取就查根目錄（spec：根目錄的子項目在切到此分頁時讀取）；樹的捲動位置寫回
    // 模組變數記下的值（被 hidden 的捲動容器 scrollTop 不保證保留，design D6）。
    if (enteringFiles) {
      if (selectedPane !== null) {
        lookupRoot();
      }
      restoreTreeScroll();
    }
    // 切到／離開「變更」分頁（git-review task 4.2）：內容與是否查詢／輪詢完全交給 git.js 自己決定
    // （它自己知道有沒有選定 pane、根目錄是不是 git repo），這裡只是進出這個分頁的通知。
    if (enteringChanges && window.cockpitGit && typeof window.cockpitGit.changesTabEntered === "function") {
      window.cockpitGit.changesTabEntered();
    }
    if (leavingChanges && window.cockpitGit && typeof window.cockpitGit.changesTabLeft === "function") {
      window.cockpitGit.changesTabLeft();
    }
    // Project 清單在 #app 內，交給 render.js 重畫時依 leftTab() 決定 hidden（design D6）。首份投影
    // 到達前 repaint() 什麼都不做，靜態占位的 Project 區塊直接跟著切換；首份投影之後的每次重畫
    // render.js 會畫出同樣的結果。
    if (typeof window.repaint === "function") {
      window.repaint();
    }
    var projects = document.querySelector('#app [data-region="projects"]');
    if (projects !== null) {
      projects.hidden = id !== LEFT_PROJECTS;
    }
    persistTabs();
  }

  if (leftTablist !== null) {
    wireTablist(leftTablist, function (tab) {
      setLeftTab(tab.getAttribute("data-left-tab"));
    });
  }

  // --- 下半部分頁區（file-review task 4.3；見檔頭「檔案分頁」）---

  var reviewTablist = reviewRoot ? reviewRoot.querySelector('[role="tablist"]') : null;
  var liveTabEl = document.getElementById(LIVE_TAB_ID);

  var DEFAULT_FILE_ICON = "file.svg"; // 對照表的預設檔案 icon（material-icons.json 的 "file"）
  var STORAGE_KEY = "cockpit.fileTabs";
  // git-review task 4.1（design D9）：v1（本 change 之前）只有檔案分頁、每筆沒有 kind 欄位；v2 每筆帶
  // kind，讀到 v1 時每筆視為 kind:"file"（見 restoreTabs()）。
  var STORAGE_VERSION = 2;
  var LOADING_FILE_TEXT = "正在讀取…";
  var NOT_READ_TEXT = "尚未讀取";
  var NEAR_BOTTOM_PX = 6; // 同 output.js 的貼底門檻
  var POLL_INTERVAL_MS = 2000; // spec「自動更新」：前一次查詢結束後 2 秒才發下一次
  var STALE_TEXT = "過期"; // 同 output.js 標題列的「過期」字樣

  // 檔案分頁專用的說法（design D10）：同一個 code 對檔案與資料夾給不同文案；其餘沿用 ERROR_TEXT。
  // root_unavailable 在檔案分頁上＝這個根目錄已沒有任何 pane 推算得出（spec「分頁還原」逐字文案）。
  var FILE_ERROR_TEXT = {
    not_found: "檔案已不存在",
    wrong_kind: "這個項目已不是檔案",
    path_outside_root: "這個檔案指向根目錄以外，不顯示內容",
    root_unavailable: "這個根目錄目前沒有任何 pane，無法讀取",
  };

  function fileErrorText(code) {
    return Object.prototype.hasOwnProperty.call(FILE_ERROR_TEXT, code) ? FILE_ERROR_TEXT[code] : errorText(code);
  }

  function fileUrl(kind, runtime, rootId, path) {
    return "/api/files/" + seg(runtime) + "/" + seg(rootId) + "/" + kind + "/" + path.split("/").map(seg).join("/");
  }

  function fileKey(runtime, rootId, path) {
    return runtime + "\n" + rootId + "\n" + path;
  }

  function baseName(path) {
    return path.slice(path.lastIndexOf("/") + 1);
  }

  function two(n) {
    return n < 10 ? "0" + n : String(n);
  }

  function clockText(ms) {
    var d = new Date(ms);
    return two(d.getHours()) + ":" + two(d.getMinutes()) + ":" + two(d.getSeconds());
  }

  // --- 分頁 kind 框架（git-review task 4.1；design D9）---
  //
  // 下半部分頁（Live Output 除外）一般化為帶 `kind` 的物件：目前只有內建的 "file"（本檔），
  // 未來 "diff"／"graph"／"rev" 由 `/app/git.js` 以 `window.cockpitGit.kinds[kind]` 提供（見下方
  // kindModuleFor()）。每個 kind 是一個「模組」，形狀：
  //
  //   {
  //     identity(fields) -> string
  //       // fields 是呼叫 openTab(kind, fields) 時給的資料（例如 file 的
  //       // { runtime, rootId, rootName, path }）；回傳這個分頁的身分鍵（design D9「分頁身分」），
  //       // 同一組邏輯上算同一個分頁的 fields 任何時候呼叫都要得到同一個字串。純函式，不能建立 DOM
  //       // 或有副作用——框架在「已打開就切換、不新增」判斷與「還原時找到已建立的分頁」都會呼叫它，
  //       // 也可能直接傳未經 deserialize() 驗證過的原始物件（只讀取自己需要的欄位）。
  //     create(fields) -> tab
  //       // 建立這個分頁的完整 DOM：包裝元素（分頁列裡的 .review-tab，含分頁本身與關閉鈕）與內容
  //       // tabpanel，自己 appendChild 進分頁列（`#review [role="tablist"]`）與 `#review`（design D6：
  //       // 這些 DOM 在 `#app` 之外，不受整頁重畫影響）。回傳的 tab 物件至少要有
  //       // `els: { wrap, tab, close, panel }`（tab 是 role="tab" 的 <button>，close 是關閉鈕，
  //       // panel 是 role="tabpanel"）；框架呼叫後才會補上 `kind`／`key`，kind 自己不用設。分頁建立
  //       // 時還不是目前分頁（不會自動選定）。其餘欄位（中繼資料、輪詢狀態……）由 kind 自己決定、自己
  //       // 讀寫，框架不碰。file kind 的完整實作見下方 FILE_KIND／createFileTab()，可直接照抄。
  //     activate(tab)   // 選填。這個分頁成為目前分頁時呼叫（framework 的 selectTab()）：開始輪詢、
  //                     // 寫回捲動位置等。沒有提供就當作沒事可做。
  //     deactivate(tab) // 選填。這個分頁被切走時呼叫：記下捲動位置、停止輪詢、作廢進行中的讀取。
  //                     // 每個 kind 要自己在這裡停掉自己的輪詢——框架不會因為「切去別的 kind」就
  //                     // 自動幫忙停（file kind 的教訓：舊版只有 file／Live Output 兩種分頁時，
  //                     // 「切到別的東西」＝「切到 Live Output」＝唯一需要 stopPolling() 的時機；
  //                     // 多了 kind 之後這個等價關係不成立了，所以 stopPolling() 現在收在
  //                     // FILE_KIND.deactivate() 裡，不再依賴「entering 是不是 null」）。
  //     dispose(tab)    // 選填。分頁被關閉時呼叫（在從陣列移除、DOM 移除之前）：釋放檢視器等資源。
  //     serialize(tab) -> object|null
  //       // 選填。存檔用（見「分頁還原」）：回傳可以存進 localStorage、之後能原封不動傳給
  //       // deserialize() 再傳給 openTab() 重建這個分頁的欄位（不含 `kind`，框架會補上）。省略這個
  //       // 方法、或回傳 null，代表這個分頁不支援還原（重新整理後就不見了）——git.js 骨架階段的三個
  //       // kind 現在就是這樣。
  //     deserialize(stored) -> fields|null
  //       // 選填。讀檔用：驗證 `stored`（persistTabs() 存的那個物件，框架呼叫時只傳除了 `kind`
  //       // 以外的部分）形狀是否合法，回傳可以傳給 openTab() 的 fields；形狀不對
  //       // 回 null（框架會 console.warn 並略過這一筆，其餘分頁照常還原，不會整份放棄）。
  //   }
  //
  // 框架提供（見下方實作）：
  //   - `openTab(kind, fields)`：已存在同身分的分頁就切過去，否則建立並切過去（掛在
  //     `window.cockpitFiles.openTab`，4.2–4.5 的「點一列開一個 diff／graph／rev 分頁」都呼叫這個）。
  //   - 分頁列共用的關閉、鍵盤、還原、持久化都是 kind-agnostic，只在需要 kind 私有行為時才呼叫上面的
  //     callback（activate／deactivate／dispose／serialize／deserialize）。
  //
  // file kind 是唯一「不透過 openTab() 開啟」的例外：openFile()（見下方，file-review task 4.4 起也給
  // Markdown 相對連結用）在 openTab() 的邏輯之上多做兩件事——已是目前分頁但上次讀取失敗時立即重試、
  // 處理 Markdown 錨點——這兩個都是 file 專屬的既有行為（不是分頁 kind 框架的一部分），所以 file kind
  // 保留自己的開啟入口，不強塞進通用的 openTab()。

  var reviewTabs = []; // 下半部全部分頁（不含 Live Output），依分頁列順序；每筆共同欄位見上方
  var fileTabSeq = 0; // file kind 分頁 DOM id 用的序號（createFileTab()；其餘 kind 各自管自己的序號）
  var restoring = false; // 還原期間不寫回 localStorage（還原完成後寫一次）

  function tabById(id) {
    for (var i = 0; i < reviewTabs.length; i += 1) {
      if (reviewTabs[i].els.tab.id === id) {
        return reviewTabs[i];
      }
    }
    return null;
  }

  function tabByKindKey(kind, key) {
    for (var i = 0; i < reviewTabs.length; i += 1) {
      if (reviewTabs[i].kind === kind && reviewTabs[i].key === key) {
        return reviewTabs[i];
      }
    }
    return null;
  }

  function currentTab() {
    return tabById(currentReviewTabId);
  }

  // kind → 模組（見上方框架說明）。"file" 內建；其餘每次都向 `window.cockpitGit` 查，不在載入時快取，
  // 所以不要求 git.js 一定比 files.js 早跑完——只要求在真的用到某個 kind（打開、還原）之前 git.js 的
  // top-level 程式碼已經執行過（index.html 仍把 git.js 排在 files.js 之前，兩者都在使用者能操作頁面
  // 之前執行完，足夠早）。
  function kindModuleFor(kind) {
    if (kind === "file") {
      return FILE_KIND;
    }
    var git = window.cockpitGit;
    var module = git && git.kinds ? git.kinds[kind] : null;
    return module && typeof module.create === "function" && typeof module.identity === "function" ? module : null;
  }

  // 找到既有分頁或建立新分頁（不切換選定）；kind 不認得（模組不存在或形狀不對）時回 null。
  function ensureTab(kind, fields) {
    var module = kindModuleFor(kind);
    if (module === null) {
      return null;
    }
    var key = module.identity(fields);
    var tab = tabByKindKey(kind, key);
    if (tab === null || tab === undefined) {
      tab = module.create(fields);
      if (!tab) {
        return null;
      }
      tab.kind = kind;
      tab.key = key;
      reviewTabs.push(tab);
    }
    return tab;
  }

  // 對外（見上方框架說明與檔尾「對外」）：找到或建立後切換過去成為目前分頁。
  function openTab(kind, fields) {
    var tab = ensureTab(kind, fields);
    if (tab !== null && tab.els.tab.id !== currentReviewTabId) {
      selectTab(tab.els.tab);
    }
    return tab;
  }

  function fileOf(ft) {
    return { runtime: ft.runtime, rootId: ft.rootId, rootName: ft.rootName, path: ft.path };
  }

  // file kind 的 create(fields)（見上方「分頁 kind 框架」）：建一個檔案分頁的 DOM（分頁＋tabpanel），
  // 加在分頁列最後面；不選定，也不加進 reviewTabs（呼叫端 ensureTab() 加）。
  // 分頁的包裝元素（.review-tab）放分頁本身（role="tab" 的 <button>）與關閉按鈕：互動元素不能巢狀
  // （button 裡不能再有 button），所以關閉按鈕是分頁的手足（files-check 契約 C4 允許的位置）。
  function createFileTab(file) {
    fileTabSeq += 1;
    var n = fileTabSeq;
    var name = baseName(file.path);
    var ft = {
      key: fileKey(file.runtime, file.rootId, file.path),
      runtime: file.runtime,
      rootId: file.rootId,
      rootName: file.rootName,
      path: file.path,
      meta: null,
      status: "idle",
      code: null,
      gen: 0,
      shown: null,
      readingSig: null,
      lastReadAt: null,
      scroll: { top: 0, pinned: false },
      pendingAnchor: null,
      viewing: false,
      closed: false,
      els: {},
    };

    var wrap = document.createElement("div");
    wrap.className = "review-tab";
    wrap.setAttribute("role", "presentation");

    var tab = document.createElement("button");
    tab.type = "button";
    tab.className = "review-tab-main";
    tab.id = "review-tab-f" + n;
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-selected", "false");
    tab.setAttribute("aria-controls", "review-panel-f" + n);
    tab.setAttribute("data-path", file.path);
    tab.tabIndex = -1;
    tab.title = file.path + "\n根目錄：" + file.rootName;
    var icon = document.createElement("img");
    icon.className = "review-tab-icon";
    icon.alt = "";
    icon.width = 16;
    icon.height = 16;
    icon.draggable = false;
    icon.src = ICON_BASE + seg(DEFAULT_FILE_ICON);
    var label = document.createElement("span");
    label.className = "review-tab-label";
    label.textContent = name;
    tab.appendChild(icon);
    tab.appendChild(label);

    var close = document.createElement("button");
    close.type = "button";
    close.className = "review-tab-close";
    close.tabIndex = -1;
    close.setAttribute("aria-label", "關閉 " + name);
    close.title = "關閉";
    close.textContent = "×";

    wrap.appendChild(tab);
    wrap.appendChild(close);
    reviewTablist.appendChild(wrap);

    var panel = document.createElement("div");
    panel.className = "review-panel file-panel";
    panel.id = "review-panel-f" + n;
    panel.setAttribute("role", "tabpanel");
    panel.setAttribute("aria-labelledby", tab.id);
    panel.hidden = true;

    // 工具列（spec「檔案分頁」）：相對路徑、最後一次成功讀取的時間、「在 VS Code 開啟」。
    var toolbar = document.createElement("div");
    toolbar.className = "file-toolbar";
    var pathEl = document.createElement("span");
    pathEl.className = "file-toolbar-path";
    pathEl.textContent = file.path;
    pathEl.title = file.path;
    // 「過期」字樣（file-review task 4.6；同 Live Output 標題列的 .output-stale-label）：只在讀取失敗、
    // 畫面上留著上一次的內容時顯示。
    var staleLabel = document.createElement("span");
    staleLabel.className = "file-stale-label";
    staleLabel.textContent = STALE_TEXT;
    staleLabel.hidden = true;
    var timeEl = document.createElement("span");
    timeEl.className = "file-toolbar-time";
    var vscode = document.createElement("a");
    vscode.className = "action-button file-vscode";
    vscode.textContent = "在 VS Code 開啟";
    vscode.hidden = true;
    toolbar.appendChild(pathEl);
    toolbar.appendChild(staleLabel);
    toolbar.appendChild(timeEl);
    toolbar.appendChild(vscode);

    var status = document.createElement("p");
    status.className = "file-status";
    status.hidden = true;

    // 檢視器的容器（也是內容的捲動容器）：viewers.js 的檢視器把內容畫在這裡（file-review task 4.4／4.5）。
    // 捲動位置隨時記進模組變數 ft.scroll（design D6；file-review task 4.4）：只在這個分頁可見時記，
    // 被 hidden 的容器 scrollTop 不可信。
    var host = document.createElement("div");
    host.className = "file-viewer-host";
    host.addEventListener("scroll", function () {
      if (!panel.hidden) {
        captureFileScroll(ft);
      }
    });

    panel.appendChild(toolbar);
    panel.appendChild(status);
    panel.appendChild(host);
    reviewRoot.appendChild(panel);

    ft.els = { wrap: wrap, tab: tab, icon: icon, close: close, panel: panel, pathEl: pathEl, staleLabel: staleLabel, timeEl: timeEl, vscode: vscode, status: status, host: host };
    renderFilePanel(ft);
    return ft;
  }

  // file kind 模組（見上方「分頁 kind 框架」）：把既有的檔案分頁邏輯接上框架的介面，行為與 git-review
  // 之前完全相同——只是原本寫死在 selectReviewTab()／closeFileTab() 裡的輪詢啟停與資源釋放，現在收進
  // activate()／deactivate()／dispose()，讓框架不必知道「檔案」這個 kind 的任何細節。
  var FILE_KIND = {
    identity: function (fields) {
      return fileKey(fields.runtime, fields.rootId, fields.path);
    },
    create: createFileTab,
    // 成為目前分頁：捲動位置寫回（design D6）、立即查一次中繼資料（見下方「自動更新」）。
    activate: function (tab) {
      restoreFileScroll(tab);
      startPolling(tab); // 內部一定先 stopPolling()，所以不管上一個目前分頁是哪個 kind 都會先收掉
    },
    // 被切走：記下捲動位置、作廢進行中的內容讀取、停止輪詢（不管接下來要換去哪個 kind——這裡是唯一
    // 一處會停掉 file kind 自己的輪詢，框架不會替它停，見上方「分頁 kind 框架」deactivate 的說明）。
    deactivate: function (tab) {
      captureFileScroll(tab);
      abandonRead(tab);
      stopPolling();
    },
    // 分頁被關閉：釋放檢視器資源（PDF 的文件、worker、render task）。
    dispose: function (tab) {
      tab.closed = true;
      tab.gen += 1; // 之後才讀完的內容一律丟棄
      var viewerHost = window.cockpitViewerHost;
      if (viewerHost && typeof viewerHost.release === "function") {
        viewerHost.release(tab.els.host);
      }
    },
    serialize: function (tab) {
      return { runtime: tab.runtime, rootId: tab.rootId, path: tab.path, rootName: tab.rootName };
    },
    deserialize: function (obj) {
      return isStoredFileTab(obj) ? { runtime: obj.runtime, rootId: obj.rootId, path: obj.path, rootName: obj.rootName } : null;
    },
  };

  // 工具列、狀態列與過期標示（只在值改變時寫 DOM：自動更新每 2 秒都會呼叫，內容沒變時不得動到節點）。
  function renderFilePanel(ft) {
    var els = ft.els;
    var timeText = ft.lastReadAt === null ? NOT_READ_TEXT : "讀取於 " + clockText(ft.lastReadAt);
    if (els.timeEl.textContent !== timeText) {
      els.timeEl.textContent = timeText;
    }
    // spec「在 VS Code 開啟」：網址是中繼資料的 vscode_uri 原值；null 時不顯示連結。
    var uri = ft.meta !== null && typeof ft.meta.vscode_uri === "string" && ft.meta.vscode_uri !== "" ? ft.meta.vscode_uri : null;
    setAttr(els.vscode, "href", uri);
    setHidden(els.vscode, uri === null);
    var text = null;
    var tone = "dim";
    if (ft.status === "error") {
      text = fileErrorText(ft.code);
      tone = "warn";
    } else if (ft.shown === null) {
      text = LOADING_FILE_TEXT; // 還沒有任何內容畫出來（查詢中或第一次讀取中）
    }
    setHidden(els.status, text === null);
    if (text !== null && els.status.textContent !== text) {
      els.status.textContent = text;
    }
    setAttr(els.status, "data-tone", tone);
    // 過期（file-review task 4.6；spec「自動更新」）：讀取失敗、畫面上留著上一次的內容時才標。沒有內容可
    // 保留（例如還原的根目錄不可用）時只顯示原因。
    var stale = ft.status === "error" && ft.shown !== null;
    if (els.panel.classList.contains("is-stale") !== stale) {
      els.panel.classList.toggle("is-stale", stale);
    }
    setHidden(els.staleLabel, !stale);
  }

  function setHidden(el, hidden) {
    if (el.hidden !== hidden) {
      el.hidden = hidden;
    }
  }

  // --- 自動更新（file-review task 4.6；見檔頭「自動更新」）---

  // 輪詢狀態：gen＝分頁身分的世代（切換／關閉分頁時加一，之後才回來的中繼資料回應丟棄）；timer＝排好的下一次；
  // controller＝進行中那一筆的 AbortController（同一時間至多一筆）。
  var poll = { gen: 0, timer: null, controller: null };

  function contentSig(meta) {
    return String(meta.size) + ":" + String(meta.modified_ms);
  }

  // 停掉目前的輪詢鏈：清掉排好的下一次、中止進行中的那一筆、世代加一。
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

  // 對目前的檔案分頁開一條新的輪詢鏈，立即查一次（spec：切換到某個檔案分頁時立即查詢一次）。
  function startPolling(ft) {
    stopPolling();
    pollMeta(ft, poll.gen);
  }

  function pollMeta(ft, gen) {
    if (gen !== poll.gen || ft.closed || currentTab() !== ft) {
      return;
    }
    var controller = new AbortController();
    poll.controller = controller;
    if (ft.status === "idle") {
      ft.status = "loading";
      renderFilePanel(ft);
    }
    getJson(fileUrl("meta", ft.runtime, ft.rootId, ft.path), controller.signal).then(function (result) {
      if (gen !== poll.gen || ft.closed) {
        return; // 切換或關閉分頁之後才回來（含被 stopPolling() 中止的那一筆）：丟棄
      }
      poll.controller = null;
      applyMeta(ft, result);
      // 這一次結束（成功或失敗）後 2 秒才排下一次。
      poll.timer = setTimeout(function () {
        poll.timer = null;
        pollMeta(ft, gen);
      }, POLL_INTERVAL_MS);
    });
  }

  function applyMeta(ft, result) {
    var body = result.ok ? result.body : null;
    if (body === null || typeof body !== "object" || typeof body.viewer !== "string") {
      ft.status = "error";
      ft.code = result.ok ? "unknown" : result.code;
      renderFilePanel(ft);
      return;
    }
    ft.meta = body;
    var iconName = typeof body.icon === "string" && body.icon !== "" ? body.icon : DEFAULT_FILE_ICON;
    setAttr(ft.els.icon, "src", ICON_BASE + seg(iconName));
    var sig = contentSig(body);
    if (sig !== (ft.viewing ? ft.readingSig : ft.shown)) {
      // 內容版本變了（或還沒讀過）：重讀。之前的失敗原因與過期標示留到這次讀取有結果時才更新。
      if (ft.status !== "error") {
        ft.status = "ok";
      }
      startRead(ft, sig);
    } else if (!ft.viewing) {
      // 畫面上已是最新版本：恢復（過期標示與原因消失）。
      ft.status = "ok";
      ft.code = null;
    }
    renderFilePanel(ft);
  }

  // 作廢正在進行的內容讀取（分頁被切走時；關閉時由 FILE_KIND.dispose() 直接加 gen）。
  function abandonRead(ft) {
    if (ft.viewing) {
      ft.gen += 1;
      ft.viewing = false;
      ft.readingSig = null;
    }
  }

  // 依中繼資料的 viewer 呼叫 viewers.js 的檢視器讀取版本 sig 的內容（見 viewers.js 檔頭「檢視器入口」；
  // file-review task 4.4／4.5 實作）。還沒有對應的檢視器時顯示占位文字，中繼資料讀到就算一次成功讀取。
  function startRead(ft, sig) {
    ft.gen += 1; // 較早那次還沒結束的讀取就此作廢
    var gen = ft.gen;
    var viewers = window.cockpitViewers || {};
    var viewer = viewers[ft.meta.viewer];
    if (typeof viewer !== "function") {
      var placeholder = document.createElement("p");
      placeholder.className = "file-placeholder";
      placeholder.textContent = "尚未實作「" + ft.meta.viewer + "」檢視器";
      ft.els.host.replaceChildren(placeholder);
      finishRead(ft, sig, { ok: true });
      return;
    }
    var ctx = {
      host: ft.els.host,
      file: fileOf(ft),
      meta: ft.meta,
      rawUrl: fileUrl("raw", ft.runtime, ft.rootId, ft.path),
      renderUrl: fileUrl("render", ft.runtime, ft.rootId, ft.path),
      rawUrlOf: function (path) {
        return fileUrl("raw", ft.runtime, ft.rootId, path);
      },
      openFile: function (path, anchor) {
        openFile({ runtime: ft.runtime, rootId: ft.rootId, rootName: ft.rootName, path: path }, anchor);
      },
      isCurrent: function () {
        return !ft.closed && ft.gen === gen;
      },
    };
    ft.viewing = true;
    ft.readingSig = sig;
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
        if (ft.closed || ft.gen !== gen) {
          return;
        }
        finishRead(ft, sig, result);
      });
  }

  function finishRead(ft, sig, result) {
    ft.viewing = false;
    ft.readingSig = null;
    if (result.ok) {
      ft.shown = sig;
      ft.lastReadAt = Date.now();
      ft.status = "ok";
      ft.code = null;
    } else {
      ft.status = "error";
      ft.code = typeof result.code === "string" ? result.code : "unknown";
    }
    renderFilePanel(ft);
    // 由相對連結開啟、要捲到錨點的分頁：內容畫好之後才捲（找不到該標題就不捲），這一次用掉就清掉。
    applyPendingAnchor(ft);
    ft.pendingAnchor = null;
  }

  // Markdown 相對連結帶的錨點（file-review task 4.4；spec「md 相對連結在分頁區開啟」）：分頁可見且內容已
  // 畫好時捲到該標題，回傳是否捲到了。捲動只動檢視器容器（viewers.js 的 cockpitMarkdownAnchor）。
  function applyPendingAnchor(ft) {
    var anchors = window.cockpitMarkdownAnchor;
    if (ft.pendingAnchor === null || ft.els.panel.hidden || !anchors || typeof anchors.reveal !== "function") {
      return false;
    }
    if (anchors.reveal(ft.els.host, ft.pendingAnchor)) {
      ft.pendingAnchor = null;
      return true;
    }
    return false;
  }

  // 切走前記下捲動位置與是否貼底，切回後寫回（design D6：被 hidden 的捲動容器 scrollTop 不保證保留）。
  function captureFileScroll(ft) {
    var host = ft.els.host;
    var gap = host.scrollHeight - host.scrollTop - host.clientHeight;
    ft.scroll = { top: host.scrollTop, pinned: host.scrollTop > 0 && gap < NEAR_BOTTOM_PX };
  }

  function restoreFileScroll(ft) {
    var host = ft.els.host;
    host.scrollTop = ft.scroll.pinned ? host.scrollHeight : ft.scroll.top;
  }

  // 讓分頁在分頁列的可視範圍內（只動分頁列自己的 scrollLeft，不捲動頁面）。
  function revealTab(el) {
    if (reviewTablist === null || el === null) {
      return;
    }
    var box = reviewTablist.getBoundingClientRect();
    var r = el.getBoundingClientRect();
    if (r.left < box.left) {
      reviewTablist.scrollLeft -= box.left - r.left;
    } else if (r.right > box.right) {
      reviewTablist.scrollLeft += Math.min(r.right - box.right, r.left - box.left);
    }
  }

  // 切換目前分頁（framework，取代原本的 selectReviewTab；見上方「分頁 kind 框架」）：leaving／entering 可能
  // 是任何 kind（或 Live Output，這時對應的 tab 物件是 null，繼續由 output.js 的 tabHidden／tabShown 管）。
  // 各 kind 自己的 activate()／deactivate() 負責自己的輪詢與狀態，框架不需要知道細節（也不需要知道
  // 「切去的是不是同一種 kind」——每個 kind 的 deactivate 在被切走時就該把自己收乾淨）。
  function selectTab(tabEl) {
    if (reviewTablist === null || tabEl.id === currentReviewTabId) {
      return;
    }
    var tabs = tabsOf(reviewTablist);
    var leaving = tabById(currentReviewTabId);
    var entering = tabById(tabEl.id);
    if (leaving !== null) {
      var leavingModule = kindModuleFor(leaving.kind);
      if (leavingModule && typeof leavingModule.deactivate === "function") {
        leavingModule.deactivate(leaving);
      }
    }
    // 契約 C3：先更新 aria-selected 與 hidden，再開始／停止輪詢（Live Output 經由 output.js 的入口；其餘
    // kind 的輪詢在下面 apply() 之後才由 activate()／deactivate() 啟停）。
    var apply = function () {
      markSelected(tabs, tabEl);
      for (var i = 0; i < tabs.length; i += 1) {
        var panel = panelOf(tabs[i]);
        if (panel !== null) {
          panel.hidden = tabs[i] !== tabEl;
        }
      }
      // 目前分頁的包裝元素（外觀：同 Live Output 分頁的目前分頁標示）與關閉按鈕的 tabindex：只有目前
      // 分頁的關閉按鈕在 Tab 順序中（分頁列本身是 roving tabindex）。
      reviewTabs.forEach(function (t) {
        var current = t.els.tab === tabEl;
        t.els.wrap.classList.toggle("is-current", current);
        t.els.close.tabIndex = current ? 0 : -1;
      });
    };
    var fromLive = currentReviewTabId === LIVE_TAB_ID;
    var toLive = tabEl.id === LIVE_TAB_ID;
    currentReviewTabId = tabEl.id;
    var live = window.liveOutput;
    if (fromLive && live && typeof live.tabHidden === "function") {
      live.tabHidden(apply);
    } else if (toLive && live && typeof live.tabShown === "function") {
      live.tabShown(apply);
    } else {
      apply();
    }
    if (entering !== null) {
      var enteringModule = kindModuleFor(entering.kind);
      if (enteringModule && typeof enteringModule.activate === "function") {
        enteringModule.activate(entering);
      }
    }
    revealTab(entering !== null ? entering.els.wrap : tabEl);
    persistTabs();
  }

  // 開檔（檔案樹點檔案列或 Enter；file-review task 4.4 起也由 Markdown 相對連結呼叫）：`file` 為
  // `{ runtime, rootId, rootName, path }`（path 為 `/` 分隔的相對路徑）。已打開的檔案切到既有分頁，
  // 不新增（spec「重複開啟不新增」）；新分頁加在最後面並成為目前分頁。anchor（可省略）：開好後捲到的
  // 標題錨點（未加 `md-` 前綴）；內容已畫好就立即捲，還在讀取就等檢視器畫完（見 finishRead()）。
  function openFile(file, anchor) {
    if (reviewTablist === null || reviewRoot === null) {
      return;
    }
    var ft = ensureTab("file", file);
    if (ft === null) {
      return;
    }
    ft.pendingAnchor = typeof anchor === "string" && anchor !== "" ? anchor : null;
    if (ft.els.tab.id === currentReviewTabId) {
      if (ft.status === "error") {
        startPolling(ft); // 已是目前分頁但上次讀取失敗：再點一次就立即重試（不等下一次輪詢）
      }
    } else {
      selectTab(ft.els.tab);
    }
    if (!applyPendingAnchor(ft) && !ft.viewing && ft.shown !== null) {
      ft.pendingAnchor = null; // 內容已畫好卻找不到該標題：不留到之後的重讀才突然捲動
    }
    persistTabs();
  }

  // 關閉分頁（framework，取代原本的 closeFileTab；任何 kind 皆適用）：關的是目前分頁時改顯示右側的分頁，
  // 沒有右側時顯示左側（第一個分頁的左側是 Live Output）。焦點在被關掉的分頁上時移到接手的分頁。
  function closeTab(tab) {
    var index = reviewTabs.indexOf(tab);
    if (index < 0) {
      return;
    }
    var neighbor = index + 1 < reviewTabs.length ? reviewTabs[index + 1].els.tab : index > 0 ? reviewTabs[index - 1].els.tab : liveTabEl;
    var hadFocus = tab.els.wrap.contains(document.activeElement);
    if (tab.els.tab.id === currentReviewTabId && neighbor !== null) {
      selectTab(neighbor); // 切走的過程會呼叫 tab 所屬 kind 的 deactivate()，輪詢等資源已在那裡停掉
    }
    var module = kindModuleFor(tab.kind);
    if (module && typeof module.dispose === "function") {
      module.dispose(tab);
    }
    reviewTabs.splice(index, 1);
    tab.els.wrap.remove();
    tab.els.panel.remove();
    if (hadFocus && neighbor !== null) {
      neighbor.focus();
    }
    persistTabs();
  }

  if (reviewTablist !== null) {
    wireTablist(reviewTablist, selectTab);
    reviewTablist.addEventListener("click", function (event) {
      var close = event.target instanceof Element ? event.target.closest(".review-tab-close") : null;
      if (close === null) {
        return;
      }
      for (var i = 0; i < reviewTabs.length; i += 1) {
        if (reviewTabs[i].els.close === close) {
          closeTab(reviewTabs[i]);
          return;
        }
      }
    });
    // 鍵盤：方向鍵／Home／End／Enter／Space 見 wireTablist；焦點在分頁上時 Delete 關閉它（關閉按鈕
    // 本身也可用 Tab 到達、以 Enter／Space 操作）。
    reviewTablist.addEventListener("keydown", function (event) {
      if (event.key !== "Delete") {
        return;
      }
      var tab = tabById(document.activeElement ? document.activeElement.id : "");
      if (tab !== null) {
        event.preventDefault();
        closeTab(tab);
      }
    });
  }

  function showLiveOutputTab() {
    if (liveTabEl !== null) {
      selectTab(liveTabEl);
    }
  }

  // --- 分頁還原（spec file-review「分頁還原」；files-check 契約 C8；git-review task 4.1 升級到 v2）---
  //
  // 存在 localStorage（鍵 STORAGE_KEY），讀寫都包 try/catch（不可用時照常運作、只是不還原）。v2 格式：
  //   { v: 2, tabs: [{ kind, ...該 kind 的 serialize() 結果 }, …]（依分頁順序）,
  //     current: { kind, ...serialize() 結果 } | null（null＝Live Output）,
  //     left: "projects" | "files" | "changes" }
  // v1（本 change 之前，只有檔案分頁）沒有 `kind` 欄位：`tabs`／`current` 每筆視為 `kind: "file"`，
  // 形狀跟 FILE_KIND.serialize() 的輸出相同（少了 rootName 的 current 例外，見 resolveCurrent()）。
  // Live Output 的 pane 選取不存（spec：不還原）。讀到不是合法 JSON、或整體形狀不對（版本無法辨識、
  // `tabs` 不是陣列、`left` 不是認得的值）時 console.warn，以沒有已打開分頁的狀態開始；單筆分頁的
  // kind 無法辨識或欄位不合法時只略過那一筆並 console.warn，其餘分頁照常還原（控制端裁決，file-review
  // 4.1 之前是整份放棄，這裡放寬——多個 kind 之後，一筆壞資料不該連累其他 kind 的分頁）。
  function persistTabs() {
    if (restoring) {
      return;
    }
    var current = tabById(currentReviewTabId);
    var data = {
      v: STORAGE_VERSION,
      tabs: serializeTabList(),
      current: current === null ? null : serializeTabEntry(current),
      left: leftTab,
    };
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch (e) {
      // 本機儲存不可用（隱私模式、配額、被停用）：不還原，其餘功能不受影響。
    }
  }

  // 一筆分頁存檔用的形狀：`{ kind, ...module.serialize(tab) }`；kind 沒有 serialize()、或它回傳非物件
  // （含 null，代表「這個分頁不支援還原」）時回傳 null（呼叫端略過，不存進 tabs 清單）。
  function serializeTabEntry(tab) {
    var module = kindModuleFor(tab.kind);
    if (!module || typeof module.serialize !== "function") {
      return null;
    }
    var fields = module.serialize(tab);
    if (fields === null || typeof fields !== "object") {
      return null;
    }
    var out = { kind: tab.kind };
    for (var k in fields) {
      if (Object.prototype.hasOwnProperty.call(fields, k)) {
        out[k] = fields[k];
      }
    }
    return out;
  }

  function serializeTabList() {
    var out = [];
    reviewTabs.forEach(function (tab) {
      var entry = serializeTabEntry(tab);
      if (entry !== null) {
        out.push(entry);
      }
    });
    return out;
  }

  function isNonEmptyString(value) {
    return typeof value === "string" && value !== "";
  }

  // 相對路徑：`/` 分隔、沒有空片段、`.` 或 `..`（跟服務端的限制同方向；只是擋掉明顯不對的值）。
  function isRelPath(value) {
    return (
      isNonEmptyString(value) &&
      value.split("/").every(function (part) {
        return part !== "" && part !== "." && part !== "..";
      })
    );
  }

  function isStoredFileTab(t) {
    return t !== null && typeof t === "object" && isNonEmptyString(t.runtime) && isNonEmptyString(t.rootId) && isRelPath(t.path) && typeof t.rootName === "string";
  }

  // 只驗整體骨架（版本、`tabs` 是不是陣列、`left` 是不是認得的值、`current` 是不是 null 或物件）；單筆
  // 分頁的形狀留到 restoreTabs() 逐筆驗證並可各自略過（見上方段落開頭的說明）。
  function isStoredState(data) {
    if (data === null || typeof data !== "object") {
      return false;
    }
    if (data.v !== 1 && data.v !== STORAGE_VERSION) {
      return false;
    }
    if (!Array.isArray(data.tabs)) {
      return false;
    }
    if (data.left !== LEFT_PROJECTS && data.left !== LEFT_FILES && data.left !== LEFT_CHANGES) {
      return false;
    }
    return data.current === null || typeof data.current === "object";
  }

  function readStoredTabs() {
    var raw;
    try {
      raw = window.localStorage.getItem(STORAGE_KEY);
    } catch (e) {
      console.warn("分頁還原：瀏覽器本機儲存不可用，不還原分頁", e);
      return null;
    }
    if (raw === null) {
      return null;
    }
    var data;
    try {
      data = JSON.parse(raw);
    } catch (e) {
      console.warn("分頁還原：本機儲存的分頁資料不是合法 JSON，以沒有已打開分頁的狀態開始");
      return null;
    }
    if (!isStoredState(data)) {
      console.warn("分頁還原：本機儲存的分頁資料形狀不對，以沒有已打開分頁的狀態開始");
      return null;
    }
    return data;
  }

  // 找出 `current`／v1 的 `current`（或任何一筆 `tabs` 紀錄）對應的既有分頁：identity() 是純函式，只讀取
  // 自己需要的欄位，所以可以直接對「未經 deserialize() 完整驗證」的原始物件呼叫（例如 v1 的 current 沒有
  // rootName，FILE_KIND.identity() 用不到這個欄位，一樣算得出來）；kind 或 identity() 本身有問題就回 null。
  function resolveCurrent(entry, fallbackKind) {
    if (entry === null || typeof entry !== "object") {
      return null;
    }
    var kind = typeof entry.kind === "string" ? entry.kind : fallbackKind;
    if (typeof kind !== "string") {
      return null;
    }
    var module = kindModuleFor(kind);
    if (module === null) {
      return null;
    }
    var key;
    try {
      key = module.identity(entry);
    } catch (e) {
      return null;
    }
    return typeof key === "string" ? tabByKindKey(kind, key) : null;
  }

  // 載入頁面時還原。還原的分頁一律保留；目前分頁是檔案分頁時開始它的自動更新（根目錄不可用時顯示
  // FILE_ERROR_TEXT.root_unavailable，依自動更新節奏重試，恢復後正常顯示；file-review task 4.6）。其餘分頁
  // 在第一次成為目前分頁時才查（之前顯示預設檔案 icon，spec「檔案 icon」）。
  function restoreTabs() {
    if (reviewTablist === null || reviewRoot === null) {
      return;
    }
    var data = readStoredTabs();
    if (data === null) {
      return;
    }
    // v1 沒有 `kind` 欄位：每一筆（含 current）都視為 "file"；v2 一定要有 `kind`，缺了就是壞資料，略過。
    var fallbackKind = data.v === 1 ? "file" : null;
    restoring = true;
    try {
      data.tabs.forEach(function (entry) {
        if (entry === null || typeof entry !== "object") {
          console.warn("分頁還原：略過形狀不對的分頁紀錄", entry);
          return;
        }
        var kind = typeof entry.kind === "string" ? entry.kind : fallbackKind;
        if (typeof kind !== "string") {
          console.warn("分頁還原：這筆分頁紀錄沒有可辨識的 kind，略過", entry);
          return;
        }
        var module = kindModuleFor(kind);
        if (module === null) {
          console.warn('分頁還原：無法辨識的分頁種類「' + kind + '」，略過', entry);
          return;
        }
        if (typeof module.deserialize !== "function") {
          console.warn('分頁還原：「' + kind + '」不支援還原，略過', entry);
          return;
        }
        var fields = module.deserialize(entry);
        if (fields === null) {
          console.warn('分頁還原：「' + kind + '」分頁紀錄欄位不合法，略過', entry);
          return;
        }
        ensureTab(kind, fields);
      });
      // git-review task 4.2：「變更」分頁現在是真的分頁，還原成它自己（不再視同「檔案」）。
      if (data.left === LEFT_FILES || data.left === LEFT_CHANGES) {
        setLeftTab(data.left);
      }
      var cur = resolveCurrent(data.current, fallbackKind);
      if (cur !== null) {
        selectTab(cur.els.tab);
      }
    } finally {
      restoring = false;
    }
  }

  // --- 對外 ---

  window.cockpitFiles = {
    leftTab: function () {
      return leftTab;
    },
    // 選取改變：「檔案」分頁可見時立即查新選取的根目錄（查到之前仍顯示上一棵樹）；在「Project」分頁時
    // 只記下選取，切到「檔案」分頁才查。同一個 pane 重選不重查（上次查詢失敗時除外）。
    paneSelected: function (runtime, paneId) {
      var same = selectedPane !== null && selectedPane.runtime === runtime && selectedPane.paneId === paneId;
      selectedPane = { runtime: runtime, paneId: paneId };
      if (!same) {
        viewGen += 1; // 丟棄上一個選取還沒回來的根目錄查詢
        view = null;
      }
      if (leftTab === LEFT_FILES && (view === null || view.status === "error")) {
        lookupRoot();
      } else {
        renderFilesPanel();
      }
      showLiveOutputTab();
    },
    // 取消選取：顯示空狀態；各根目錄的展開狀態留在 rootStates，重新選回時還原。
    paneCleared: function () {
      selectedPane = null;
      viewGen += 1;
      view = null;
      renderFilesPanel();
    },
    // 每次整頁重畫（render.js paint()）交來目前投影的 pane 集合 `[{ runtime, paneId, cwd }]`（最終修正波
    // F1；見檔頭「檔案樹」）：選定 pane 的 cwd 與上次查根目錄時不同、且「檔案」分頁可見時重查根目錄。
    // pane 不在投影中時不動（消失由 output.js 處理）；cwd 沒變時不發任何請求。
    setKnownPanes: function (panes) {
      var next = new Map();
      for (var i = 0; i < panes.length; i += 1) {
        next.set(panes[i].runtime + "\n" + panes[i].paneId, typeof panes[i].cwd === "string" ? panes[i].cwd : null);
      }
      paneCwds = next;
      if (selectedPane === null || view === null || leftTab !== LEFT_FILES) {
        return;
      }
      var cwd = selectedCwd();
      if (cwd !== undefined && cwd !== view.cwd) {
        lookupRoot(true);
      }
    },
    // git-review task 4.1（design D9）：找到或建立指定 kind 的分頁並切過去。4.2–4.5 的「變更」面板／diff／
    // Git Graph／某版本分頁由 `/app/git.js` 的 kind 模組（`window.cockpitGit.kinds`）實作，透過這個入口開啟
    // （見檔頭與上方「分頁 kind 框架」的介面說明）；"file" 也認得，但 file 專屬的開檔行為（Markdown 錨點、
    // 已是目前分頁時的重試）走 openFile()，不是這個入口。kind 不認得或建立失敗時回 null。
    openTab: openTab,
  };

  restoreTabs();
})();
