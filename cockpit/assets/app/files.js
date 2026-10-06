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
//   - 錯誤依回應本體的 `code` 查 ERROR_TEXT 顯示文案（design D10；文字隨介面語言，字典在 i18n.js），不顯示 HTTP 狀態碼或 API 路徑；
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
//   - 檔案分頁變成可見時立即查中繼資料，之後依自動更新節奏重查（見下方「自動更新」），取得 icon、
//     `vscode_uri`、`viewer`，內容版本（size／modified_ms）與畫面上的不同時依 `viewer` 呼叫 viewers.js
//     重讀（見 viewers.js 檔頭「檢視器入口」）。Live Output 為目前分頁時不查。
//   - 關閉目前分頁 → 右側，沒有右側 → 左側（Live Output 不可關閉）；焦點在檔案分頁上時 Delete 也可關閉。
//     關閉並排組合中的分頁時改依並排規則（見下方「並排」）。
//   - 並排（file-split-view task 3.1；spec「檔案並排」）：每個檔案分頁的關閉鈕左側有並排鈕（aria-pressed），
//     最多 3 個檔案分頁同時可見；加入、替換焦點欄、移出與關閉的狀態轉換見 selectState()／leaveSplit()／
//     toggleSplit()，可見集合由 visibleTabs() 推導。
//   - 切走前記下內容的捲動位置與是否貼底，切回後寫回（design D6）；分頁可見期間捲動位置也隨時記在
//     ft.scroll（file-review task 4.4）。
//   - Markdown 相對連結經由檢視器的 ctx.openFile(path, anchor) 開檔；帶錨點時等內容畫好才捲到該標題
//     （file-review task 4.4；見 openFile()、applyPendingAnchor()）。
//   - 已打開的分頁、目前分頁與左欄目前分頁存在 localStorage（見下方「分頁還原」），載入時還原。
//
// 自動更新（file-review task 4.6；spec file-review「自動更新」「分頁還原」；design D6、D7）：
//   - 只查「可見的檔案分頁」的中繼資料（file-split-view task 2.2 起以 isVisible() 判斷；沒有並排時就是
//     目前分頁）。每個分頁各有一條輪詢鏈，狀態放在自己的 ft.poll（file-split-view task 2.3；design D3）：
//     變成可見時立即查一次；每次查詢結束（成功或失敗）後 2 秒才排下一次（POLL_INTERVAL_MS，setTimeout
//     串接，不用 setInterval），同一個分頁同一時間至多一個進行中。分頁變成不可見或被關閉時
//     stopPolling(ft)：清掉它排好的下一次、abort 它進行中的那一筆，並把它的輪詢世代（ft.poll.gen）加一，
//     之後才回來的回應一律丟棄。只動這個分頁自己的鏈，其他可見分頁的輪詢不受影響。Live Output 為目前
//     分頁、或沒有檔案分頁時沒有任何輪詢鏈存在。
//   - 兩種世代分開（控制端裁決）：ft.poll.gen 是「這一段可見期間」的世代，只在分頁變成可見、不可見或
//     被關閉時變（error 分頁的立即重試也會重開一條）；ft.gen 是「內容讀取」的世代（檢視器的
//     ctx.isCurrent() 以它判斷），只在開始下一次重讀、分頁被切走或關閉時變。輪詢本身不動 ft.gen，所以
//     讀一個大 PDF 的期間照常輪詢不會把這次讀取當成過期丟掉。
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

  var t = window.cockpitI18n.t;
  var tn = window.cockpitI18n.tn;

  var filesRoot = document.getElementById("files");
  var reviewRoot = document.getElementById("review");

  // --- 模組狀態 ---

  var leftTab = LEFT_PROJECTS;
  var currentReviewTabId = LIVE_TAB_ID; // 目前分頁＝焦點（aria-selected、roving tabindex、持久化的 current）
  // 並排（file-split-view task 2.2；design D1）：分頁區「可見」的分頁不再等於目前分頁，由 visibleTabs()
  // 從目前分頁與並排組合推導（見下方「可見集合」）。狀態轉換（加入、替換、移出、關閉）見下方「並排」
  // （file-split-view task 3.1；design D9）。
  var SPLIT_MAX = 3; // spec「檔案並排」：最多 3 個檔案分頁同時並排
  var splitTabs = []; // 並排組合：檔案 kind 物件，長度 0 或 2～3，順序就是欄位順序
  var splitFocus = null; // 焦點欄：並排組合非空時恆為其中一員，空時為 null（目前分頁離開並排組合時，恢復並排靠它）
  var shownTabs = [null]; // 上一次 applyVisibility() 套用的可見集合；null＝Live Output（載入時只有它可見）
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

  // onActivate(tab, event)：event 是觸發選定的 click 事件（file-split-view task 3.3；design D7）。分頁區的呼叫端讀
  // event.ctrlKey 判斷 Ctrl＋點選，左欄的呼叫端不看它。
  function wireTablist(tablist, onActivate) {
    tablist.addEventListener("click", function (event) {
      var tab = event.target instanceof Element ? event.target.closest('[role="tab"]') : null;
      if (tab !== null && tablist.contains(tab)) {
        onActivate(tab, event);
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

  // design D10：依錯誤本體的 `code` 查表（spec「檔案端點的共同規則」代碼表），不以 HTTP 狀態碼或 API
  // 路徑開頭。表裡存字典鍵（ui-language：文字隨介面語言，見 i18n.js 的 files.error.*）；`not_found`／
  // `wrong_kind` 依目標是根目錄查詢還是資料夾給不同說法（見 errorText()）。
  var ERROR_TEXT = {
    forbidden_source: "files.error.forbiddenSource",
    method_not_allowed: "files.error.methodNotAllowed",
    bad_request: "files.error.badRequest",
    runtime_unknown: "files.error.runtimeUnknown",
    pane_unknown: "files.error.paneUnknown",
    no_root: "files.error.noRoot",
    root_unavailable: "files.error.rootUnavailable",
    path_outside_root: "files.error.pathOutsideRoot",
    not_found: "files.error.notFound",
    wrong_kind: "files.error.wrongKind",
    too_large: "files.error.tooLarge",
    not_markdown: "files.error.notMarkdown",
    io_error: "files.error.ioError",
    network: "files.error.network",
    timeout: "files.error.timeout",
  };
  var UNKNOWN_ERROR_KEY = "files.error.unknown";

  function errorText(code) {
    var key = Object.prototype.hasOwnProperty.call(ERROR_TEXT, code) ? ERROR_TEXT[code] : UNKNOWN_ERROR_KEY;
    return t(key, { seconds: REQUEST_TIMEOUT_MS / 1000 });
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
    refreshButton.textContent = t("files.tree.refresh");
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
    treeEl.setAttribute("aria-label", t("files.tree.aria"));
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
        rows.push({ key: noteKey + "loading", level: level, text: path === "" ? t("files.tree.loadingRoot") : t("files.tree.loadingDir"), tone: "dim" });
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
        rows.push({ key: noteKey + "more", level: level, text: tn("files.tree.more", hiddenCount), tone: "dim" });
      } else if (listing.entries.length === 0) {
        rows.push({ key: noteKey + "empty", level: level, text: t("files.tree.emptyDir"), tone: "dim" });
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
      status = t("files.tree.loadingRoot");
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
  var NEAR_BOTTOM_PX = 6; // 同 output.js 的貼底門檻
  var POLL_INTERVAL_MS = 2000; // spec「自動更新」：前一次查詢結束後 2 秒才發下一次

  // 檔案分頁專用的說法（design D10）：同一個 code 對檔案與資料夾給不同文案；其餘沿用 ERROR_TEXT。
  // root_unavailable 在檔案分頁上＝這個根目錄已沒有任何 pane 推算得出（spec「分頁還原」逐字文案）。
  // 同 ERROR_TEXT，表裡存字典鍵。
  var FILE_ERROR_TEXT = {
    not_found: "files.fileError.notFound",
    wrong_kind: "files.fileError.wrongKind",
    path_outside_root: "files.fileError.pathOutsideRoot",
    root_unavailable: "files.fileError.rootUnavailable",
  };

  function fileErrorText(code) {
    return Object.prototype.hasOwnProperty.call(FILE_ERROR_TEXT, code) ? t(FILE_ERROR_TEXT[code]) : errorText(code);
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
  //     activate(tab)   // 選填。這個分頁變成可見時呼叫（framework 的 applyVisibility()；file-split-view
  //                     // task 2.2 起由可見集合的比對觸發，不再是「成為目前分頁」——並排中切換焦點欄時
  //                     // 兩邊都仍可見，不會呼叫）：開始輪詢、寫回捲動位置等。沒有提供就當作沒事可做。
  //                     // 不參與並排的 kind（git 類）可見就等於是目前分頁，兩種意思對它們相同。
  //     deactivate(tab) // 選填。這個分頁變成不可見時呼叫（同上；關閉一個仍可見的分頁時也會先呼叫，
  //                     // 之後才 dispose()）：記下捲動位置、停止輪詢、作廢進行中的讀取。
  //                     // 每個 kind 要自己在這裡停掉自己的輪詢——框架不會因為「切去別的 kind」就
  //                     // 自動幫忙停（file kind 的教訓：舊版只有 file／Live Output 兩種分頁時，
  //                     // 「切到別的東西」＝「切到 Live Output」＝唯一需要 stopPolling() 的時機；
  //                     // 多了 kind 之後這個等價關係不成立了，所以 stopPolling(tab) 現在收在
  //                     // FILE_KIND.deactivate() 裡，不再依賴「entering 是不是 null」）。
  //     dispose(tab)    // 選填。分頁被關閉時呼叫（在從陣列移除、DOM 移除之前）：停掉自己的輪詢、
  //                     // 釋放檢視器等資源。不要假設 deactivate() 一定先跑過（file-split-view task 2.3）。
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
  // Markdown 相對連結用）在 openTab() 的邏輯之上多做兩件事——已經可見但上次讀取失敗時立即重試、
  // 處理 Markdown 錨點——這兩個都是 file 專屬的既有行為（不是分頁 kind 框架的一部分），所以 file kind
  // 保留自己的開啟入口，不強塞進通用的 openTab()。

  var reviewTabs = []; // 下半部全部分頁（不含 Live Output），依分頁列順序；每筆共同欄位見上方
  var fileTabSeq = 0; // file kind 分頁 DOM id 用的序號（createFileTab()；其餘 kind 各自管自己的序號）
  // 還原期間不寫回 localStorage。還原完成後也不另外寫一次：儲存的紀錄維持原樣，直到下一次操作觸發 persistTabs() 才被
  // 覆寫，所以被略過的分頁與不合法的並排資料會留到那時（每次載入多一則警告）。刻意不在還原後補寫：那會提早丟掉新版
  // 才認得的 kind（file-split-view task 5.2 最終審查）。
  var restoring = false;

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

  // --- 可見集合（file-split-view task 2.2；design D1、D2）---
  //
  // 「目前分頁」（currentReviewTabId，焦點）與「可見」是兩個概念：可見集合由 visibleTabs() 從目前分頁與
  // 並排組合推導，不另存一份會不同步的狀態。所有會改變可見集合的入口（選定分頁、開檔、關閉、還原……）
  // 改完狀態後都呼叫 applyVisibility()，由它比對新舊集合：對離開的分頁呼叫 kind 的 deactivate()、對進入
  // 的分頁呼叫 activate()，兩邊都有的分頁什麼都不做（並排中切換焦點欄時其他欄不重新讀取，由這個結構保證）。
  // tabpanel 的 hidden 只由 applyVisibility() 寫，所以「hidden ⇔ 不在可見集合」恆成立。

  // 並排顯示的寬度門檻（design D5；spec「檔案並排」的「窄視窗」）：視窗至少 760 CSS px 才並排顯示，否則只顯示焦點欄。
  // 760 與 style.css 的單欄斷點是同一個數值，改一邊要一起改（CSS 與 JS 無法共用常數）。visibleTabs() 與下面的
  // change 監聽共用這一個 MediaQueryList（file-split-view task 3.5）。
  var SPLIT_WIDTH_QUERY = window.matchMedia("(min-width: 760px)");

  // 推導可見集合（純函式）：目前分頁在並排組合裡、而且視窗夠寬（SPLIT_WIDTH_QUERY）時是整個並排組合，其餘情況只有
  // 目前分頁；Live Output 是 [null]。
  function visibleTabs() {
    var current = tabById(currentReviewTabId);
    if (current !== null && splitTabs.indexOf(current) >= 0 && SPLIT_WIDTH_QUERY.matches) {
      return splitTabs.slice();
    }
    return [current];
  }

  // 視窗寬度跨過門檻時重新套用可見集合（file-split-view task 3.5；design D5）。只靠 CSS 藏欄不夠：藏起來的欄仍在可見
  // 集合裡，會繼續輪詢。經 applyVisibility() 比對進出：變窄時非焦點欄 deactivate()（停輪詢、中止進行中的查詢），變寬時
  // 這些欄 activate()（立即查詢一次），#review 的 data-split 與各欄標記也一併更新。不在並排中時可見集合不變，
  // 不會觸發任何 activate()／deactivate()。
  SPLIT_WIDTH_QUERY.addEventListener("change", function () {
    if (reviewTablist !== null) {
      applyVisibility();
    }
  });

  // 這個分頁此刻是否可見（以上一次 applyVisibility() 套用的集合為準，跟 tabpanel 的 hidden 一致）。
  function isVisible(tab) {
    return shownTabs.indexOf(tab) >= 0;
  }

  // 把可見集合與目前分頁的標示套到 DOM 上，並觸發各 kind 的生命週期（design D2）。順序沿用舊版
  // selectTab()：先對離開的分頁 deactivate()，再更新 aria-selected 與 hidden（契約 C3：先更新
  // aria-selected，再開始輪詢），最後對進入的分頁 activate()。Live Output 進出可見集合時，DOM 更新包在
  // output.js 的 tabHidden()／tabShown() 回呼裡（它要在面板藏起來之前記下捲動位置、顯示之後才請求）。
  function applyVisibility() {
    var prev = shownTabs;
    var next = visibleTabs();
    var leaving = prev.filter(function (entry) {
      return next.indexOf(entry) < 0;
    });
    var entering = next.filter(function (entry) {
      return prev.indexOf(entry) < 0;
    });
    shownTabs = next; // 先換：activate() 裡立即發出的查詢以 isVisible() 判斷要不要繼續
    leaving.forEach(function (entry) {
      callKindHook(entry, "deactivate");
    });
    var tabs = tabsOf(reviewTablist);
    var current = tabById(currentReviewTabId);
    var currentEl = current !== null ? current.els.tab : liveTabEl;
    var shownEls = next.map(function (entry) {
      return entry !== null ? entry.els.tab : liveTabEl;
    });
    // 並排顯示＝可見集合有 2 個以上（visibleTabs() 只在並排中且視窗夠寬時回整個並排組合，其餘情況只有一個）。
    var splitShown = next.length >= 2;
    var apply = function () {
      markSelected(tabs, currentEl);
      for (var i = 0; i < tabs.length; i += 1) {
        var panel = panelOf(tabs[i]);
        if (panel !== null) {
          panel.hidden = shownEls.indexOf(tabs[i]) < 0;
        }
      }
      // 目前分頁的包裝元素（外觀：同 Live Output 分頁的目前分頁標示）與關閉按鈕的 tabindex：只有目前
      // 分頁的關閉按鈕在 Tab 順序中（分頁列本身是 roving tabindex）。這兩項是焦點，不是可見。
      // 並排組合的標記（file-split-view task 3.1、3.3；design D7）：包裝元素的 data-split-col＝欄位編號（1 起算，
      // style.css 依它畫數字徽章；選定 Live Output 等「不在並排中」時也保留，並排組合沒有解除）；並排鈕與「並排第 N 欄」
      // 說明見 paintSplitControls()。
      reviewTabs.forEach(function (entry) {
        var isCurrent = entry === current;
        entry.els.wrap.classList.toggle("is-current", isCurrent);
        entry.els.close.tabIndex = isCurrent ? 0 : -1;
        var col = splitTabs.indexOf(entry);
        setAttr(entry.els.wrap, "data-split-col", col >= 0 ? String(col + 1) : null);
        if (entry.els.split) {
          paintSplitControls(entry, col, isCurrent);
        }
        // 並排版面（file-split-view task 3.2；design D4）：並排顯示中，各欄面板的 CSS order＝欄位編號（分頁列
        // 是 0，排在最前），焦點欄的面板帶 data-split-focus（外框由 style.css 畫）。只改 order，不搬動面板的
        // DOM：搬動 html 檢視器的 <iframe> 會讓它重新載入。
        var shownCol = splitShown ? next.indexOf(entry) : -1;
        var order = shownCol >= 0 ? String(shownCol + 1) : "";
        if (entry.els.panel.style.order !== order) {
          entry.els.panel.style.order = order;
        }
        setAttr(entry.els.panel, "data-split-focus", shownCol >= 0 && isCurrent ? "" : null);
      });
      // #review 的 data-split＝實際可見的並排欄數（file-split-view task 3.2；控制端裁決 C）；沒有並排顯示時
      // （不在並排中、或窄視窗只剩焦點欄）移除，回到原本的 flex column。
      if (reviewRoot !== null) {
        setAttr(reviewRoot, "data-split", splitShown ? String(next.length) : null);
      }
    };
    var fromLive = prev.indexOf(null) >= 0;
    var toLive = next.indexOf(null) >= 0;
    var live = window.liveOutput;
    if (fromLive && !toLive && live && typeof live.tabHidden === "function") {
      live.tabHidden(apply);
    } else if (toLive && !fromLive && live && typeof live.tabShown === "function") {
      live.tabShown(apply);
    } else {
      apply();
    }
    entering.forEach(function (entry) {
      callKindHook(entry, "activate");
    });
  }

  // 呼叫 tab 所屬 kind 的 activate()／deactivate()／dispose()（選填，沒有就當作沒事可做）；null（Live Output）
  // 略過。
  function callKindHook(tab, name) {
    if (tab === null) {
      return;
    }
    var module = kindModuleFor(tab.kind);
    if (module && typeof module[name] === "function") {
      module[name](tab);
    }
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
      // 這個分頁自己的輪詢鏈（見下方「自動更新」；file-split-view task 2.3）。
      poll: { gen: 0, timer: null, controller: null },
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
    // 分頁的 title 與並排時的路徑說明（pathDesc）共用同一段文字。title 只在這裡寫一次：切換介面語言會重新載入頁面、
    // 分頁重建，路徑與根目錄名稱在分頁存活期間不變。
    var titleText = t("files.tab.title", { path: file.path, root: file.rootName });
    tab.title = titleText;
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

    // 並排鈕（file-split-view task 3.1、3.3；design D7；spec「檔案並排」的並排鈕）：關閉鈕左側的切換鈕。只有檔案分頁有。
    // aria-pressed（是否在並排組合中）、aria-disabled＋title（停用與原因）與 tabindex（只有目前分頁的在 Tab 順序中，
    // 同關閉鈕）都由 applyVisibility() 依狀態寫（paintSplitControls()）；顯示時機（目前分頁或滑鼠移上去）由 style.css 管。
    var split = document.createElement("button");
    split.type = "button";
    split.className = "review-tab-split";
    split.tabIndex = -1;
    split.setAttribute("aria-pressed", "false");
    split.setAttribute("aria-label", t("files.tab.splitNamed", { name: name }));
    split.textContent = "◫";
    // 「並排第 N 欄」說明（file-split-view task 3.3；design D7）：在並排組合中時，分頁以 aria-describedby 指到這裡。
    // 用 hidden：只給輔助技術透過 aria-describedby 讀（被直接參照的隱藏節點仍計入說明），本身不出現在畫面與無障礙樹，
    // 分頁列（tablist）裡也就不會多出一段不是分頁的文字。
    var colDesc = document.createElement("span");
    colDesc.id = tab.id + "-col";
    colDesc.hidden = true;
    // 完整路徑＋根目錄（file-split-view task 3.3 修正第 1 輪）：分頁一旦帶 aria-describedby，Chrome 就不再拿 title 當說明；
    // 並排時 aria-describedby 依序指到 colDesc 與這裡，兩個根目錄各自的 README.md 並排時才分得出是哪一個。
    var pathDesc = document.createElement("span");
    pathDesc.id = tab.id + "-path";
    pathDesc.hidden = true;
    pathDesc.textContent = titleText;

    var close = document.createElement("button");
    close.type = "button";
    close.className = "review-tab-close";
    close.tabIndex = -1;
    close.setAttribute("aria-label", t("files.tab.closeNamed", { name: name }));
    close.title = t("files.tab.close");
    close.textContent = "×";

    wrap.appendChild(tab);
    wrap.appendChild(split);
    wrap.appendChild(close);
    wrap.appendChild(colDesc);
    wrap.appendChild(pathDesc);
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
    staleLabel.textContent = t("files.toolbar.stale");
    staleLabel.hidden = true;
    var timeEl = document.createElement("span");
    timeEl.className = "file-toolbar-time";
    var vscode = document.createElement("a");
    vscode.className = "action-button file-vscode";
    vscode.textContent = t("files.toolbar.vscode");
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
    // 被 hidden 的容器 scrollTop 不可信（並排中的非焦點欄也可見，照樣記；file-split-view task 2.2）。
    var host = document.createElement("div");
    host.className = "file-viewer-host";
    host.addEventListener("scroll", function () {
      if (isVisible(ft)) {
        captureFileScroll(ft);
      }
    });

    panel.appendChild(toolbar);
    panel.appendChild(status);
    panel.appendChild(host);
    reviewRoot.appendChild(panel);

    ft.els = { wrap: wrap, tab: tab, icon: icon, split: split, colDesc: colDesc, pathDesc: pathDesc, close: close, panel: panel, pathEl: pathEl, staleLabel: staleLabel, timeEl: timeEl, vscode: vscode, status: status, host: host };
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
    // 變成可見（file-split-view task 2.2 起由可見集合的比對觸發）：捲動位置寫回（design D6）、立即查一次
    // 中繼資料（見下方「自動更新」）。
    activate: function (tab) {
      restoreFileScroll(tab);
      startPolling(tab); // 只開這個分頁自己的鏈；其他仍可見的分頁照常輪詢（file-split-view task 2.3）
    },
    // 變成不可見（被切走、或關閉一個仍可見的分頁時在 dispose() 之前）：記下捲動位置、作廢進行中的內容
    // 讀取、停止這個分頁自己的輪詢（不管接下來要換去哪個 kind——框架不會替它停，見上方「分頁 kind 框架」
    // deactivate 的說明）。
    deactivate: function (tab) {
      captureFileScroll(tab);
      abandonRead(tab);
      stopPolling(tab);
    },
    // 分頁被關閉：停掉它的輪詢、釋放檢視器資源（PDF 的文件、worker、render task）。
    dispose: function (tab) {
      tab.closed = true;
      // 不依賴 deactivate() 先停過（file-split-view task 2.3；task 2.1 盤點 #18）：並排後關閉一個可見但
      // 不是焦點欄的分頁，未必會先走 deactivate()。這裡再停一次：清掉計時器、abort 進行中的查詢。
      stopPolling(tab);
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
    var timeText = ft.lastReadAt === null ? t("files.toolbar.notRead") : t("files.toolbar.readAt", { time: clockText(ft.lastReadAt) });
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
      text = t("files.status.loading"); // 還沒有任何內容畫出來（查詢中或第一次讀取中）
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

  // 輪詢狀態放在每個檔案分頁自己的 ft.poll（file-split-view task 2.3；design D3）：gen＝這一段可見期間的世代
  // （變成可見、不可見或被關閉時加一，之後才回來的中繼資料回應丟棄）；timer＝排好的下一次；controller＝進行中
  // 那一筆的 AbortController（同一個分頁同一時間至多一筆）。並排時每個可見分頁各有一條，互不覆寫。

  function contentSig(meta) {
    return String(meta.size) + ":" + String(meta.modified_ms);
  }

  // 停掉 ft 自己的輪詢鏈：清掉排好的下一次、中止進行中的那一筆、世代加一。不碰其他分頁的鏈。
  function stopPolling(ft) {
    var poll = ft.poll;
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

  // 對可見的檔案分頁重開它自己的輪詢鏈，立即查一次（spec：變成可見時立即查詢一次；openFile() 對 error
  // 分頁的立即重試也走這裡）。只先停掉 ft 自己的舊鏈，其他可見分頁的輪詢照常（file-split-view task 2.3）。
  function startPolling(ft) {
    stopPolling(ft);
    pollMeta(ft, ft.poll.gen);
  }

  // 繼續條件是「可見」，不是「目前分頁」（file-split-view task 2.2；spec「自動更新」：每個可見的檔案分頁
  // 各自查詢，並排中的非焦點欄也要查）。世代、計時器與 controller 全部讀寫 ft.poll（file-split-view task 2.3）。
  function pollMeta(ft, gen) {
    var poll = ft.poll;
    if (gen !== poll.gen || ft.closed || !isVisible(ft)) {
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
        return; // 這個分頁變成不可見或被關閉之後才回來（含被 stopPolling(ft) 中止的那一筆）：丟棄
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
      placeholder.textContent = t("files.viewer.notImplemented", { kind: ft.meta.viewer });
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
  // 畫好時捲到該標題，回傳是否捲到了。捲動只動檢視器容器（viewers.js 的 cockpitMarkdownAnchor）。「可見」
  // 含並排中的非焦點欄（file-split-view task 2.2）。
  function applyPendingAnchor(ft) {
    var anchors = window.cockpitMarkdownAnchor;
    if (ft.pendingAnchor === null || !isVisible(ft) || !anchors || typeof anchors.reveal !== "function") {
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

  // 切換目前分頁（framework，取代原本的 selectReviewTab；見上方「分頁 kind 框架」）：進出可見集合的分頁
  // 可能是任何 kind（或 Live Output，這時對應的 tab 物件是 null，繼續由 output.js 的 tabHidden／tabShown
  // 管；見 applyVisibility()）。各 kind 自己的 activate()／deactivate() 負責自己的輪詢與狀態，框架不需要
  // 知道細節（也不需要知道「切去的是不是同一種 kind」——每個 kind 的 deactivate 在變成不可見時就該把
  // 自己收乾淨）。
  function selectTab(tabEl) {
    if (reviewTablist === null || tabEl.id === currentReviewTabId) {
      return;
    }
    // file-split-view task 2.2（design D2）：先改狀態（目前分頁與並排組合，見 selectState()），再由
    // applyVisibility() 比對可見集合的進出、更新標示並觸發各 kind 的 activate()／deactivate()。
    selectState(tabById(tabEl.id));
    currentReviewTabId = tabEl.id;
    applyVisibility();
    var entering = tabById(tabEl.id);
    revealTab(entering !== null ? entering.els.wrap : tabEl);
    persistTabs();
  }

  // --- 並排（file-split-view task 3.1；spec file-review「檔案並排」；design D1、D9）---
  //
  // 狀態：splitTabs（並排組合，長度 0 或 2～3）、splitFocus（焦點欄，並排組合非空時恆為其中一員）與
  // currentReviewTabId（目前分頁）。「並排中」＝目前分頁在並排組合裡，這時目前分頁一定是 splitFocus。
  // 下面三個函式只改狀態，呼叫端改完後呼叫 applyVisibility()（design D2）。只有 file kind 能進並排組合
  // （入口驗 kind：selectState() 的替換與 toggleSplit() 的加入），所以 git 類分頁「可見就是目前分頁」的前提不變。

  // 目前是否「並排中」。
  function inSplitView() {
    var current = tabById(currentReviewTabId);
    return current !== null && splitTabs.indexOf(current) >= 0;
  }

  // 按 tab 的並排鈕會不會動作（spec「加入」：沒有並排組合、而且目前分頁不是 tab 以外的檔案分頁時停用；design D7）。
  // 已有並排組合時一律可用（加入、替換或移出）。只有檔案分頁有並排鈕。
  function splitAvailable(tab) {
    if (tab === null || tab.kind !== "file") {
      return false;
    }
    if (splitTabs.length > 0) {
      return true;
    }
    var current = tabById(currentReviewTabId);
    return current !== null && current.kind === "file" && current !== tab;
  }

  // 依狀態寫並排鈕與「並排第 N 欄」說明（file-split-view task 3.3；design D7）。col：tab 在並排組合中的位置（-1＝不在）。
  //   - aria-pressed：是否在並排組合中。
  //   - 停用：aria-disabled="true"（不用 disabled 屬性，滑鼠移上去才看得到 title），title 說明要先選另一個檔案分頁；
  //     可用時移除 aria-disabled，title 為「並排」。
  //   - tabindex：只有目前分頁的並排鈕在 Tab 順序中（同關閉鈕）。
  //   - 在並排組合中時，分頁以 aria-describedby 依序指到「並排第 N 欄」與完整路徑＋根目錄（同 title）；不在時移除，
  //     這時輔助技術照舊以 title 當說明。
  function paintSplitControls(tab, col, isCurrent) {
    var els = tab.els;
    var enabled = splitAvailable(tab);
    setAttr(els.split, "aria-pressed", col >= 0 ? "true" : "false");
    setAttr(els.split, "aria-disabled", enabled ? null : "true");
    setAttr(els.split, "title", t(enabled ? "files.tab.split" : "files.tab.splitDisabled"));
    els.split.tabIndex = isCurrent ? 0 : -1;
    var desc = col >= 0 ? t("files.tab.splitCol", { n: col + 1 }) : "";
    if (els.colDesc.textContent !== desc) {
      els.colDesc.textContent = desc;
    }
    setAttr(els.tab, "aria-describedby", col >= 0 ? els.colDesc.id + " " + els.pathDesc.id : null);
  }

  // 分頁區的選定入口（file-split-view task 3.3；design D7）：點選分頁（wireTablist）與 Ctrl＋Enter 共用。
  // wantSplit（按住 Ctrl）且該分頁的並排鈕可用時，效果等同按並排鈕；否則（不是檔案分頁、並排鈕停用、沒按 Ctrl）是
  // 一般的選定（spec「並排鈕」）。
  function activateReviewTab(tabEl, wantSplit) {
    var tab = tabById(tabEl.id);
    if (wantSplit && splitAvailable(tab)) {
      toggleSplit(tab);
    } else {
      selectTab(tabEl);
    }
  }

  // 選定分頁（target；Live Output 為 null）之前的並排狀態轉換（spec「替換」「不在並排中」）：
  //   - target 在並排組合中：它成為焦點欄（回到並排中，或只是換焦點欄）。
  //   - 並排中、target 是不在並排組合中的檔案分頁：它取代焦點欄的分頁，位置不變，並成為焦點欄；被取代的分頁
  //     留在分頁列。點選分頁、從檔案樹開檔、點 md 相對連結（含新開的分頁）都經過 selectTab() 走到這裡。
  //   - 其他（Live Output、git 類分頁、不在並排中時選定未並排的檔案分頁）：並排組合與焦點欄保留不動，只以單欄
  //     顯示 target（由 visibleTabs() 推導）。
  // 呼叫端接著把 currentReviewTabId 設成 target。
  function selectState(target) {
    if (target !== null && splitTabs.indexOf(target) >= 0) {
      splitFocus = target;
    } else if (target !== null && target.kind === "file" && inSplitView()) {
      splitTabs[splitTabs.indexOf(splitFocus)] = target;
      splitFocus = target;
    }
  }

  // 把 tab 移出並排組合（按並排鈕移出、或關閉並排組合中的分頁；design D9 的 4 步）。不在並排組合中時不動作。
  function leaveSplit(tab) {
    var index = splitTabs.indexOf(tab);
    if (index < 0) {
      return;
    }
    // 1. 先記下移出前是否並排中。
    var wasInSplit = inSplitView();
    // 2. 移出的是焦點欄：焦點欄改為右側欄，沒有右側時為左側欄。
    if (tab === splitFocus) {
      splitFocus = index + 1 < splitTabs.length ? splitTabs[index + 1] : splitTabs[index - 1];
    }
    splitTabs.splice(index, 1);
    // 3. 只剩一個分頁時解除並排組合（splitTabs 的長度因此恆為 0 或 2～3）。
    var remaining = null;
    if (splitTabs.length < 2) {
      remaining = splitTabs.length === 1 ? splitTabs[0] : null;
      splitTabs = [];
      splitFocus = null;
    }
    // 4. 目前分頁只在移出前是並排中時才跟著改：改成新的焦點欄；並排組合已解除時改成剩下的那個分頁。移出前
    //    不是並排中（例如目前是 Live Output）時目前分頁不動。
    if (wasInSplit) {
      var next = splitFocus !== null ? splitFocus : remaining;
      if (next !== null) {
        currentReviewTabId = next.els.tab.id;
      }
    }
  }

  // 按並排鈕（spec「加入」「移出」）。tab 在並排組合中就移出；不在就加入：
  //   - 沒有並排組合：目前分頁是 tab 以外的檔案分頁 Y 時，並排組合成為 Y、tab 兩欄；否則按了不動作（並排鈕此時
  //     為停用狀態，見 splitAvailable()）。
  //   - 已有並排組合且未滿 3 個：tab 加到最右欄。
  //   - 已滿 3 個：tab 取代焦點欄的分頁，位置不變；被取代的分頁留在分頁列。
  //   只要 tab 有加入，它就成為焦點欄與目前分頁（並排中）。
  function toggleSplit(tab) {
    // 停用條件只寫在 splitAvailable() 一處（file-split-view task 5.2 最終審查）：通過之後，沒有並排組合時目前分頁一定是
    // tab 以外的檔案分頁。
    if (!splitAvailable(tab) || reviewTabs.indexOf(tab) < 0) {
      return;
    }
    var currentBefore = currentReviewTabId;
    if (splitTabs.indexOf(tab) >= 0) {
      leaveSplit(tab);
    } else {
      if (splitTabs.length === 0) {
        splitTabs = [tabById(currentReviewTabId), tab];
      } else if (splitTabs.length < SPLIT_MAX) {
        splitTabs.push(tab);
      } else {
        splitTabs[splitTabs.indexOf(splitFocus)] = tab;
      }
      splitFocus = tab;
      currentReviewTabId = tab.els.tab.id;
    }
    applyVisibility();
    // 只有目前分頁真的換了才捲（file-split-view task 3.3 修正第 1 輪）：不在並排中移出成員時目前分頁不變，分頁列不該
    // 被捲回目前分頁（使用者正看著被按的那一顆）。
    if (currentReviewTabId !== currentBefore) {
      var now = tabById(currentReviewTabId);
      revealTab(now !== null ? now.els.wrap : liveTabEl);
    }
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
    // 呼叫前就已經可見、而且上次讀取失敗：再點一次就立即重試（不等下一次輪詢）。條件是「可見」不是
    // 「目前分頁」（file-split-view task 2.2；design D2）：並排中點一個可見但不是焦點欄的 error 欄，除了
    // 切焦點也要重試。呼叫前不可見的分頁不必重試，selectTab() 讓它進入可見集合時 activate() 會立即查一次。
    var retry = isVisible(ft) && ft.status === "error";
    if (ft.els.tab.id !== currentReviewTabId) {
      selectTab(ft.els.tab);
    }
    if (retry) {
      startPolling(ft); // 只重開 ft 自己的鏈，不影響其他可見分頁的輪詢（file-split-view task 2.3）
    }
    if (!applyPendingAnchor(ft) && !ft.viewing && ft.shown !== null) {
      ft.pendingAnchor = null; // 內容已畫好卻找不到該標題：不留到之後的重讀才突然捲動
    }
    persistTabs();
  }

  // 關閉分頁（framework，取代原本的 closeFileTab；任何 kind 皆適用）：
  //   - 被關閉的分頁在並排組合中（file-split-view task 3.1；design D9；spec「移出」）：跟按並排鈕移出共用
  //     leaveSplit()，目前分頁依並排規則決定，不套用「改為顯示右側分頁」。
  //   - 其餘情況：關的是目前分頁時改顯示右側的分頁，沒有右側時顯示左側（第一個分頁的左側是 Live Output）。
  // 焦點在被關掉的分頁上時移到接手的分頁：目前分頁因此改變時是新的目前分頁，否則是右側相鄰（沒有右側時為左側）的分頁。
  function closeTab(tab) {
    var index = reviewTabs.indexOf(tab);
    if (index < 0) {
      return;
    }
    var hadFocus = tab.els.wrap.contains(document.activeElement);
    var successor;
    var revealEl = null; // 並排分支且目前分頁換了：接手的分頁（目前分頁）要捲進分頁列的視野，同 toggleSplit()
    var neighbor = index + 1 < reviewTabs.length ? reviewTabs[index + 1].els.tab : index > 0 ? reviewTabs[index - 1].els.tab : liveTabEl;
    if (splitTabs.indexOf(tab) >= 0) {
      var currentBefore = currentReviewTabId;
      leaveSplit(tab);
      // 由可見集合的比對處理進出（design D2）：被關閉的分頁若仍可見就離開集合、呼叫 deactivate()（停輪詢、
      // 中止進行中的查詢），之後才 dispose()；並排解除時剩下的那欄留在集合裡，什麼都不做。
      applyVisibility();
      if (currentReviewTabId !== currentBefore) {
        // 目前分頁改變（並排中關閉的是焦點欄）：目前分頁改為新的焦點欄（或並排解除後剩下的那個），焦點與可視範圍都跟過去。
        var now = tabById(currentReviewTabId);
        successor = now !== null ? now.els.tab : liveTabEl;
        revealEl = now !== null ? now.els.wrap : liveTabEl;
      } else {
        // 目前分頁不變（不在並排中，或並排中關閉的不是焦點欄；file-split-view task 3.3 修正第 1 輪）：同下方「關的不是
        // 目前分頁」，焦點移到相鄰分頁、不捲動分頁列（移到 Live Output 會讓 focus() 把分頁列捲回最左）。
        successor = neighbor;
      }
    } else {
      successor = neighbor;
      if (tab.els.tab.id === currentReviewTabId && successor !== null) {
        selectTab(successor); // 切走的過程會呼叫 tab 所屬 kind 的 deactivate()，輪詢等資源已在那裡停掉
      }
    }
    // dispose() 不假設 deactivate() 已經跑過，file kind 會在這裡再停一次自己的輪詢（file-split-view task 2.3）。
    callKindHook(tab, "dispose");
    reviewTabs.splice(index, 1);
    tab.els.wrap.remove();
    tab.els.panel.remove();
    // 在移除被關閉分頁之後才量位置（file-split-view task 3.3；審查第 1 項）。
    if (revealEl !== null) {
      revealTab(revealEl);
    }
    if (hadFocus && successor !== null) {
      successor.focus();
    }
    persistTabs();
  }

  if (reviewTablist !== null) {
    // 按住 Ctrl 點選分頁＝按它的並排鈕（並排鈕停用或不是檔案分頁時等同一般選定；file-split-view task 3.3）。
    wireTablist(reviewTablist, function (tabEl, event) {
      activateReviewTab(tabEl, !!(event && event.ctrlKey));
    });
    reviewTablist.addEventListener("click", function (event) {
      var target = event.target instanceof Element ? event.target : null;
      var close = target !== null ? target.closest(".review-tab-close") : null;
      // 並排鈕（file-split-view task 3.1）：只有檔案分頁有，toggleSplit() 另外再驗 kind。停用（aria-disabled）時照樣呼叫，
      // toggleSplit() 在停用條件下本來就不動作（file-split-view task 3.3）。
      var split = target !== null ? target.closest(".review-tab-split") : null;
      if (close === null && split === null) {
        return;
      }
      for (var i = 0; i < reviewTabs.length; i += 1) {
        if (close !== null && reviewTabs[i].els.close === close) {
          closeTab(reviewTabs[i]);
          return;
        }
        if (split !== null && reviewTabs[i].els.split === split) {
          toggleSplit(reviewTabs[i]);
          return;
        }
      }
    });
    // 鍵盤：方向鍵／Home／End／Enter／Space 見 wireTablist；焦點在分頁上時 Delete 關閉它（關閉按鈕
    // 本身也可用 Tab 到達、以 Enter／Space 操作）。
    // 焦點在分頁上時 Ctrl＋Enter＝按它的並排鈕（file-split-view task 3.3；design D7；spec「鍵盤加入並排」），規則同
    // Ctrl＋點選。一定要 preventDefault()：<button> 的 Enter 預設會再轉成一次 click，多跑一次選定。
    reviewTablist.addEventListener("keydown", function (event) {
      if (event.key === "Enter" && event.ctrlKey) {
        var focused = document.activeElement;
        if (focused !== null && focused.getAttribute("role") === "tab" && reviewTablist.contains(focused)) {
          event.preventDefault();
          activateReviewTab(focused, true);
        }
        return;
      }
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

  // 讓某一欄成為焦點欄的兩種操作（file-split-view task 3.4；design D6；spec「焦點欄的標示與切換」）：在那一欄內按下
  // 滑鼠（pointerdown），或焦點進入那一欄的 iframe（window blur＋輪詢，見下）。用 Tab 鍵把焦點移到 md 欄的連結等父文件
  // 裡的元素，不會切換焦點欄（沒有聽 focusin）；用 Tab 鍵把焦點移進 iframe 則會，因為走的是 iframe 那條路。
  //
  // pointerdown：掛在 #review 的 capture 階段，早於欄內各自的處理。pointerdown 先換焦點欄，接著的 click 若是 md 相對連結
  // 開檔，就照 spec「替換」換掉這一欄，也就是「在哪一欄點的連結，就在哪一欄開」；PDF 工具列等其他點擊照常進行。
  // 不呼叫 focus()、不 preventDefault()：使用者是要在那一欄捲動、選字或點連結，不能搶走焦點或擋掉預設動作；滑鼠操作後
  // 由程式 focus() 也會被 Chrome 判成 :focus-visible、留下外框。
  // 只在並排中、按在可見的並排欄面板內、而且不是焦點欄時動作：分頁列、Live Output 面板、單欄顯示（含窄視窗只剩焦點欄）
  // 都不動（判斷見 splitColumnAt()／focusColumnAt()）。
  //
  // iframe：html 檢視器的內容在 sandbox iframe 裡（不允許腳本），iframe 內按下滑鼠不會傳到這個文件。
  //   - 焦點從父文件進入 iframe：父文件的 window 收到 blur，activeElement 變成那個 <iframe>（task 3.4 修正第 1 輪）。
  //   - 焦點從一個 iframe 直接移到另一個 iframe：父文件收不到 blur／focus／focusin／focusout，只有 activeElement 默默改變
  //     （審查實測，修正第 2 輪）。所以焦點在某個並排欄的 iframe 裡時，以短週期輪詢 activeElement（控制端裁決）。
  // 兩者都經 followIframeFocus()：activeElement 是非焦點並排欄的 iframe 就切換；之後只要 activeElement 仍是某個並排欄
  // 的 iframe、在並排中、頁面沒有隱藏，就排下一次檢查，否則立刻停。平常沒有計時器在跑。
  //   - blur 後等 setTimeout(0) 才讀 activeElement：blur 當下 activeElement 不保證已經換成接手焦點的 iframe。
  //   - 焦點回到父文件（window 的 focus）或頁面隱藏時立刻停止輪詢；頁面重新顯示時檢查一次（焦點可能仍在 iframe 裡）。
  //   - 切到別的應用程式：焦點原本在父文件裡時 activeElement 不是 iframe，不動作；焦點原本就在 iframe 裡時，輪詢照常，
  //     但 activeElement 沒變，也不動作。切回來後點另一個 html 欄的 iframe，由輪詢接手。
  //   - 只認 target 為 window 的 blur／focus：元素的 blur／focus 不冒泡，不 capture 就不會收到，這裡再擋一次。
  //   - 已知缺口（file-split-view task 5.2 最終審查記錄，不修）：焦點所在的 iframe 被換掉（html 檔更新後檢視器換新的
  //     iframe）時父文件收不到任何事件，輪詢在下一次檢查時停止，之後從一個 iframe 直接移到另一個 iframe 就不會切換焦點欄；
  //     在父文件內任意按一下即恢復。窄視窗只剩焦點欄、焦點在它的 iframe 裡時輪詢會持續，但不會切換任何東西。
  var IFRAME_POLL_MS = 200;
  var iframePoll = null;

  // node 所在的並排欄（並排中、可見的並排欄面板內）；不在任何並排欄裡、或不在並排中時回 null。
  function splitColumnAt(node) {
    if (!inSplitView() || !(node instanceof Node)) {
      return null;
    }
    for (var i = 0; i < splitTabs.length; i += 1) {
      var tab = splitTabs[i];
      if (isVisible(tab) && tab.els.panel.contains(node)) {
        return tab;
      }
    }
    return null;
  }

  // node 所在的並排欄，而且不是焦點欄（要切換過去的那一欄）；否則回 null。
  function focusColumnAt(node) {
    var tab = splitColumnAt(node);
    return tab !== null && tab !== splitFocus ? tab : null;
  }

  function stopIframePoll() {
    if (iframePoll !== null) {
      clearTimeout(iframePoll);
      iframePoll = null;
    }
  }

  function scheduleIframeCheck(delay) {
    stopIframePoll();
    iframePoll = setTimeout(followIframeFocus, delay);
  }

  // blur 與輪詢共用的判斷：焦點在某個並排欄的 iframe 裡才繼續，是非焦點欄就切換。
  function followIframeFocus() {
    iframePoll = null;
    var active = document.activeElement;
    if (document.hidden || active === null || active.tagName !== "IFRAME" || splitColumnAt(active) === null) {
      return;
    }
    var tab = focusColumnAt(active);
    if (tab !== null) {
      selectTab(tab.els.tab);
    }
    scheduleIframeCheck(IFRAME_POLL_MS);
  }

  if (reviewRoot !== null) {
    reviewRoot.addEventListener(
      "pointerdown",
      function (event) {
        var tab = focusColumnAt(event.target);
        if (tab !== null) {
          selectTab(tab.els.tab);
        }
      },
      true
    );
    window.addEventListener("blur", function (event) {
      if (event.target === window) {
        scheduleIframeCheck(0);
      }
    });
    window.addEventListener("focus", function (event) {
      if (event.target === window) {
        stopIframePoll();
      }
    });
    document.addEventListener("visibilitychange", function () {
      if (document.hidden) {
        stopIframePoll();
      } else {
        scheduleIframeCheck(0);
      }
    });
  }

  // 選定 pane 時切到 Live Output。經 selectTab() → applyVisibility()，可見集合的進出一次處理：之前可見的
  // 分頁（並排時整組）一起 deactivate()，Live Output 的顯示包在 tabShown() 裡（file-split-view task 2.2）。
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
  //     left: "projects" | "files" | "changes",
  //     split: [索引, …]（選填）, splitFocus: 索引（選填） }
  // v1（本 change 之前，只有檔案分頁）沒有 `kind` 欄位：`tabs`／`current` 每筆視為 `kind: "file"`，
  // 形狀跟 FILE_KIND.serialize() 的輸出相同（少了 rootName 的 current 例外，見 resolveCurrent()）。
  // Live Output 的 pane 選取不存（spec：不還原）。讀到不是合法 JSON、或整體形狀不對（版本無法辨識、
  // `tabs` 不是陣列、`left` 不是認得的值）時 console.warn，以沒有已打開分頁的狀態開始；單筆分頁的
  // kind 無法辨識或欄位不合法時只略過那一筆並 console.warn，其餘分頁照常還原（控制端裁決，file-review
  // 4.1 之前是整份放棄，這裡放寬——多個 kind 之後，一筆壞資料不該連累其他 kind 的分頁）。
  //
  // 並排組合（file-split-view task 3.6；design D8）：版號維持 v2，加兩個選填欄位。`split` 是並排組合依欄位順序、
  // `splitFocus` 是焦點欄，值都是「寫入的 `tabs` 陣列」中的索引（serializeTabList() 略過不存的分頁之後才算）；
  // 沒有並排組合時兩個欄位都不寫。不升版號是為了回滾：舊版的 isStoredState() 不看多出來的欄位，回滾只失去並排。
  // 還原時索引一律對照儲存位置，不對照還原後的位置（見 restoreSplit()）。
  function persistTabs() {
    if (restoring) {
      return;
    }
    var current = tabById(currentReviewTabId);
    var list = serializeTabList();
    var data = {
      v: STORAGE_VERSION,
      tabs: list.entries,
      current: current === null ? null : serializeTabEntry(current),
      left: leftTab,
    };
    var split = splitTabs.map(function (tab) {
      return list.positions.has(tab) ? list.positions.get(tab) : -1;
    });
    // 並排成員都是檔案分頁，一定存得下來；萬一有成員沒存（-1），寧可不存並排，也不存一份指錯的索引。
    if (split.length >= 2 && split.indexOf(-1) < 0) {
      data.split = split;
      data.splitFocus = list.positions.get(splitFocus);
    }
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

  // 依分頁順序序列化所有分頁：entries 是要寫入的 `tabs` 陣列（略過 serializeTabEntry() 回傳 null 的分頁），
  // positions 是「分頁物件 → 它在 entries 中的索引」（略過之後才算；並排組合的索引用它，file-split-view task 3.6）。
  function serializeTabList() {
    var entries = [];
    var positions = new Map();
    reviewTabs.forEach(function (tab) {
      var entry = serializeTabEntry(tab);
      if (entry !== null) {
        positions.set(tab, entries.length);
        entries.push(entry);
      }
    });
    return { entries: entries, positions: positions };
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

  function isStoredFileTab(tab) {
    return tab !== null && typeof tab === "object" && isNonEmptyString(tab.runtime) && isNonEmptyString(tab.rootId) && isRelPath(tab.path) && typeof tab.rootName === "string";
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

  // 還原並排組合（file-split-view task 3.6；design D8；spec「分頁還原」）。restoredAt：儲存位置 → 還原出的分頁。
  // 沒有 `split` 欄位（v1、或 v2 沒有並排組合）時什麼都不做。不合法時忽略整個並排組合、console.warn，其餘狀態照常
  // 還原：不是陣列、長度不在 2～SPLIT_MAX、索引不是整數或越界、指到還原時被略過的位置、指到非檔案分頁、有重複（含兩個
  // 儲存位置還原成同一個分頁）。焦點欄不在並排組合中（含沒有 `splitFocus` 欄位）時改用第一欄；目前分頁在並排組合中時
  // 由呼叫端的 selectTab() 改成目前分頁。只改狀態，呼叫端接著走 applyVisibility()。
  function restoreSplit(data, restoredAt) {
    if (!Object.prototype.hasOwnProperty.call(data, "split")) {
      return;
    }
    // 警告訊息直接寫在 console.warn 的第一個引數裡（i18n-check 只放行那裡的中文；介面文字才進字典）。
    var stored = data.split;
    if (!Array.isArray(stored)) {
      console.warn("分頁還原：並排組合不合法（不是陣列），忽略並排組合", stored);
      return;
    }
    if (stored.length < 2 || stored.length > SPLIT_MAX) {
      console.warn("分頁還原：並排組合不合法（分頁數 " + stored.length + " 不在 2～" + SPLIT_MAX + "），忽略並排組合", stored);
      return;
    }
    var tabs = [];
    for (var i = 0; i < stored.length; i += 1) {
      var index = stored[i];
      var tab = Number.isInteger(index) ? restoredAt.get(index) : undefined;
      if (!Number.isInteger(index) || index < 0 || index >= data.tabs.length) {
        console.warn("分頁還原：並排組合不合法（索引 " + JSON.stringify(index) + " 不存在），忽略並排組合", stored);
        return;
      }
      if (tab === undefined) {
        console.warn("分頁還原：並排組合不合法（索引 " + index + " 的分頁在還原時被略過），忽略並排組合", stored);
        return;
      }
      if (tab.kind !== "file") {
        console.warn("分頁還原：並排組合不合法（索引 " + index + " 不是檔案分頁），忽略並排組合", stored);
        return;
      }
      if (tabs.indexOf(tab) >= 0) {
        console.warn("分頁還原：並排組合不合法（索引 " + index + " 與前面的欄重複），忽略並排組合", stored);
        return;
      }
      tabs.push(tab);
    }
    var focus = Number.isInteger(data.splitFocus) ? restoredAt.get(data.splitFocus) : undefined;
    splitTabs = tabs;
    splitFocus = focus !== undefined && tabs.indexOf(focus) >= 0 ? focus : tabs[0];
  }

  // 載入頁面時還原。還原的分頁一律保留；可見的檔案分頁開始它的自動更新（根目錄不可用時顯示
  // FILE_ERROR_TEXT.root_unavailable，依自動更新節奏重試，恢復後正常顯示；file-review task 4.6）。其餘分頁
  // 在第一次變成可見時才查（之前顯示預設檔案 icon，spec「檔案 icon」；file-split-view task 2.2）。
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
    // 儲存位置 → 還原出的分頁（file-split-view task 3.6；design D8）：被略過的位置沒有對應，並排組合的索引查這張表。
    var restoredAt = new Map();
    restoring = true;
    try {
      data.tabs.forEach(function (entry, position) {
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
        var tab = ensureTab(kind, fields);
        if (tab !== null) {
          restoredAt.set(position, tab);
        }
      });
      // git-review task 4.2：「變更」分頁現在是真的分頁，還原成它自己（不再視同「檔案」）。
      if (data.left === LEFT_FILES || data.left === LEFT_CHANGES) {
        setLeftTab(data.left);
      }
      restoreSplit(data, restoredAt);
      // 並排組合要在選定目前分頁之前還原：目前分頁在並排組合中時，selectTab() 經 selectState() 把焦點欄改成它（spec
      // 「分頁還原」：目前分頁在並排組合中時，焦點欄一律為目前分頁），再由 applyVisibility() 一次套好可見集合、#review 的
      // data-split 與輪詢（file-split-view task 3.6）。
      var cur = resolveCurrent(data.current, fallbackKind);
      if (cur !== null) {
        selectTab(cur.els.tab);
      } else {
        // 目前分頁維持 Live Output：可見集合不變，但還原出的分頁要依狀態寫一次並排鈕等標示（file-split-view task 3.3）。
        applyVisibility();
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
