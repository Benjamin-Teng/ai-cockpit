// render.js：收到整張投影圖就整頁重畫（spec cockpit-dashboard「畫面整頁重畫」「Factory
// Floor」；設計文件 §8.3；change pipeline-projection design D9；外框改版見 direction-01-visual
// design D2、task 2.1）。renderState(state, ui) 是純函數：只讀 state／ui、回傳一份新建的
// DocumentFragment（各區塊平鋪、根節點帶 data-region；design D2）、不讀寫任何全域變數或既有
// DOM。會碰全域（document、window）的只有檔尾的 paint／window.onState／window.repaint／
// window.onChannel、#app 上唯一一個 focusin listener（鍵盤焦點進 Factory Floor 時補捲，見檔尾），以及 el() 內建立節點用的 document.createElement()／
// document.createDocumentFragment()、鈴鐺圖示用的 document.createElementNS()（節點工廠呼叫，不是讀寫既有 DOM）。
//
// 桌面通知（desktop-launch-notify task 3.3；design D7、D8）：頂列多一顆通知鈴鐺
// （`data-action="notify-settings"`，事件由 actions.js 委派、交給 notify.js 開關設定面板）；
// `window.onState` 重畫後呼叫 `window.cockpitNotify.observe(state)`（notify.js 沒載入時略過）。
// 設定面板是 notify.js 建在 body 底下、#app 之外的節點，不在這裡的整頁重畫範圍內。
//
// ui 是選填的第二參數，缺省＝無改綁模式、無錯誤訊息、無選取（形狀見 actions.js 的
// uiSnapshot：`{ rebind: null | { project, workstream }, error: null | string,
// selected: null | { runtime, paneId } }`）。畫面操作的按鈕（spec「畫面操作」；task 5.3）在
// 這裡只輸出 `<button>` 與 `data-action`／`data-project`／`data-task`／`data-workstream`／
// `data-runtime`／`data-pane` 屬性，**不綁任何 listener**——事件委派、fetch 與 UI 狀態都在
// actions.js（design D9）。
//
// Live Output 的選取（spec live-output「選定一個 pane」；design D8；task 5.3／fix round 1
// R15）：未 exited 且不在改綁模式的 pane 列輸出 `data-action="select-pane"`（可鍵盤觸發，見
// 下方 tabindex／role）；改綁模式期間整列不可點選（R15：不呈現可點選樣式、不輸出
// `data-action`／`tabindex`，只留「綁定到這裡」按鈕可點）。被選定的那一列另加 `.selected`
// class——這個標示獨立於可不可點選，改綁模式期間若原本就有選取，標示仍要保留。workstream
// 列首在 `binding.state === "bound"` 時多一顆「看輸出」（`data-action="select-bound-pane"`），
// 不受改綁模式影響。`render.js` 每次重畫（`paint()`）後都會呼叫
// `window.liveOutput.setKnownPanes(...)`，交出目前投影裡還存在的 pane 集合。
//
// agent 狀態只有 working／blocked／done／idle／unknown 五種 class（符號＋文字＋色彩，見
// agentState()）；任何其他字串（例如未來協定加的新值）一律落在 unknown 的次要文字色與虛線環，
// 並把原字串保留在文字與 title 裡，不會讓整頁壞掉。Factory Floor 的 task 節點另有自己的六種狀態色（running／blocked／ready／pending／
// failed／completed），未知字串同樣落在暗灰、不會壞掉整頁。所有文字一律用 textContent 寫入，
// 不用 innerHTML，避免把資料當成標記解析。
//
// 焦點還原（spec cockpit-dashboard「畫面整頁重畫」；spec live-output「選定一個 pane」；task
// focus-fix；同時解掉專案已知的使用性問題 M3）：`paint()` 在 `replaceChildren` 前後各比對一次
// 焦點所在元素的「身分」（`data-action` ＋當下全部 `data-*`），把焦點還原到新樹裡代表同一個
// 對象的元素上，找不到就留在 `<body>`。詳見檔尾 captureFocusIdentity／findByFocusIdentity／
// restoreFocus 上方註解。

(function () {
  "use strict";

  // 介面文字一律經字典（i18n.js，ui-language task 2.1；key 命名見該檔檔頭）：t(key, params) 具名佔位符、
  // tn(key, n, params) 依數量選 .one／.other。i18n.js 是 index.html 第一個載入的腳本，這裡載入時就拿得到。
  var t = window.cockpitI18n.t;
  var tn = window.cockpitI18n.tn;
  var tMsg = window.cockpitI18n.tMsg;

  var KNOWN_STATUSES = ["working", "blocked", "done", "idle", "unknown"];

  function statusClass(status) {
    return KNOWN_STATUSES.indexOf(status) !== -1 ? "status-" + status : "status-unknown";
  }

  function orDash(value) {
    return value === null || value === undefined || value === "" ? "—" : value;
  }

  function el(tag, className, text) {
    var node = document.createElement(tag);
    if (className) {
      node.className = className;
    }
    if (text !== undefined && text !== null) {
      node.textContent = text;
    }
    return node;
  }

  function statusBadge(status) {
    var cls = statusClass(status);
    var badge = el("span", "status " + cls, status);
    if (cls === "status-unknown") {
      badge.title = status;
    }
    return badge;
  }

  // agent 狀態＝符號＋文字＋色彩（design D4 agent 對照表；direction-01-visual task 3.3）：
  // 外層 .agent-state 把符號與文字排成一組；符號是 CSS 畫的 8px 圓（.agent-dot，aria-hidden、
  // 沒有文字——不用 ●／○ 字元，避免和 task pending 的 ○ 混淆），形狀與顏色由
  // .agent-dot-<狀態> 決定（見 style.css）；文字仍是原本的 .status 節點（class、文字、未知字串
  // 的 title 都不變，whatever-check.js 與 reconnect-check.js 讀的就是它）。符號是 .status 的
  // 手足、不是子節點，.status 的 textContent 維持精確等於 agent_status 字串。
  function agentState(status) {
    var cls = statusClass(status);
    var wrap = el("span", "agent-state");
    var dot = el("span", "agent-dot agent-dot-" + cls.slice("status-".length));
    dot.setAttribute("aria-hidden", "true");
    wrap.appendChild(dot);
    wrap.appendChild(statusBadge(status));
    return wrap;
  }

  // 連線三色（design D4「連線」對照表：connected→--ok、connecting→--warn、
  // disconnected→--bad）共用於頂列 runtime 燈號與底列通道狀態（design D4「連線配色也適用
  // 底列通道狀態」；direction-01-visual task 2.3）。未知字串（理論上不會發生——連線狀態與
  // channel.js 的通道狀態都是受控的固定枚舉，不是外部自由輸入）落在 --text-dim，跟專案其餘
  // 「未知不破壞畫面」的慣例一致。
  var KNOWN_CONN_STATES = ["connected", "connecting", "disconnected"];

  function connStateClass(prefix, state) {
    return KNOWN_CONN_STATES.indexOf(state) !== -1 ? prefix + state : prefix + "unknown";
  }

  // 最新通道（瀏覽器→cockpit）狀態（Codex C1，direction-01-visual task 2.3 fix round 1）：
  // 過去 renderChannelIndicator() 每次整頁重畫都寫死 "connected"，若通道已經斷線、之後又有
  // 任何非 WS 觸發的 repaint()（例如使用者操作、鍵盤 Enter 觸發的 UI-only 重畫，經
  // window.repaint() 呼叫 paint()），底列會被錯誤畫回綠色——這不是同步競態，是「整頁重畫的
  // 資料來源只看 latestState（task 的投影），完全不知道通道當下的真實狀態」這個結構性缺陷。
  // 修法：模組層級保存 latestChannelState，只由 window.onChannel 寫入；renderChannelIndicator()
  // 與 renderTopbar()（design I2，見下方 renderRuntimeLamp）改讀這個值，不再寫死。初值採
  // "connecting"，跟 index.html 的靜態占位一致——這個初值實務上不會被讀到：paint() 只會在
  // window.onState 被呼叫後才執行，而 onState 只可能在 /ws 的第一則訊息抵達後才被 channel.js
  // 呼叫，那之前 socket.onopen 一定已經呼叫過 onChannel("connected")（見 channel.js），所以
  // 第一次 paint() 讀到 latestChannelState 時，它已經被 onChannel 正確設成 "connected"。
  var latestChannelState = "connecting";

  // M4／N5（設計審核，task 2.3 fix round 1／fix round 2）：頂列燈號 title 講清楚是哪一段連線
  // （跟底列「瀏覽器→cockpit 服務」的通道區分開），通道非 connected 時額外標「（最後已知）」
  // ——斷線期間畫面已經寫「最後已知 connected」，滑鼠停留卻只看到舊的說明文字會誤導使用者以為
  // 這是即時資料。renderRuntimeLamp() 與 window.onChannel 都呼叫這個函式產生同一份文字，兩條
  // 路徑不會對不齊（沿用 I2 的「模組層級狀態＋兩處讀同一份」設計）。
  function lampTitle(runtimeId, connState, channelState) {
    return t(channelState === "connected" ? "render.lamp.title" : "render.lamp.titleStale", {
      runtime: runtimeId,
      state: connState,
    });
  }

  // I2（使用者決定，direction-01-visual task 2.3 fix round 1；顏色依 Codex fix round 1
  // review／N3 改為 fix round 2 的 --text-dim，見下方）：頂列 runtime 燈號的符號＋id＋狀態
  // 文字，跟這個 runtime「自己」的連線狀態（win／wsl 對 HERDR）有關；但當瀏覽器與 cockpit
  // 服務之間的通道（latestChannelState）不是 connected 時，這份投影可能已經是舊資料，所以
  // 整顆燈號（含符號、id、「最後已知」、狀態文字全部四個子節點）改用 --text-dim（見
  // style.css `[data-region="topbar"]:not([data-channel-state="connected"]) .runtime-lamp`
  // 與 `.runtime-lamp-stale { color: inherit; }`），並在狀態文字前面插入「最後已知」。這段
  // 文字一律輸出到 DOM（design 慣例：狀態相關文字放 DOM，不用 CSS content），用 CSS 依
  // topbar 的 data-channel-state 屬性切換顯示／隱藏——`window.onChannel` 只要更新那個屬性
  // （不重畫）就能立刻套用，下一次整頁重畫時 renderTopbar() 也會用同一份 latestChannelState
  // 產生同樣的屬性，兩條路徑不會對不齊。「最後已知」刻意是 .runtime-lamp-state 的**手足**
  // 節點、不是它的子節點或文字內容——data-conn-state 節點的 textContent 必須維持精確等於
  // 連線狀態字串本身（R1 的斷言用 .trim() 精確比對，不能被這段前綴污染）。
  function renderRuntimeLamp(runtime) {
    var connState = runtime.connection.state;
    var lamp = el("span", "runtime-lamp " + connStateClass("runtime-lamp-", connState));
    lamp.setAttribute("data-runtime", runtime.id);
    lamp.title = lampTitle(runtime.id, connState, latestChannelState);

    // 符號用 CSS 畫的 8px 圖形（.conn-symbol，aria-hidden；M3 使用者決定：connected 實心圓、
    // connecting 空心圓、disconnected 叉，全部 CSS 畫、不用字元，跟底列 .channel-status-dot
    // 共用同一組形狀規則，見 style.css）。
    var dot = el("span", "runtime-lamp-dot conn-symbol");
    dot.setAttribute("aria-hidden", "true");
    lamp.appendChild(dot);

    // id 是等寬字四類之一（design D11：時間、id、數值、Live Output）。fix round 3／
    // Ruling R26（換設計，拿掉 JS 量測與「+N」徽章）：id 一律留在 DOM 裡、不整顆移除——
    // 固定一屏空間不夠時交給 style.css 的 flex＋ellipsis 先縮 id，再不夠就讓
    // .topbar-runtimes 橫向捲動（overflow-x: auto），完整 id 一律留在 title。
    lamp.appendChild(el("span", "runtime-lamp-id", runtime.id));

    // I2：「最後已知」一律輸出，預設由 CSS 隱藏（topbar 的 data-channel-state="connected"
    // 時），通道非 connected 時才顯示——見上方函式註解。
    lamp.appendChild(el("span", "runtime-lamp-stale", t("render.lamp.stale")));

    var stateEl = el("span", "runtime-lamp-state", connState);
    // design D11：狀態文字一律用無襯線，不套等寬——跟 .runtime-lamp-id 刻意不同字體。
    stateEl.setAttribute("data-conn-state", "");
    lamp.appendChild(stateEl);

    return lamp;
  }

  // 頂列（design D2；direction-01-visual task 2.3）：產品名稱降為面板標題級字級
  // （design D10；task 2.2 已把 .app-name 的 font-size 收斂成 --fs-panel，這裡不重複改）＋
  // 每個 runtime 一個連線燈號。通道狀態與 version 搬到底列（design D4「連線配色也適用底列
  // 通道狀態」；下方 renderStatusbarRegion()）。data-channel-state 屬性（I2，fix round 1）
  // 掛在這個節點上，供 CSS 判斷要不要把燈號調暗＋顯示「最後已知」；window.onChannel 不重畫時
  // 直接改這個屬性，這裡則是每次整頁重畫時用 latestChannelState 重新產生同樣的值，兩者一致。
  function renderTopbar(state) {
    var topbar = el("div", "topbar");
    topbar.id = "topbar";
    // design D2 的外框定位依據（direction-01-visual task 2.1）：`.shell` 用
    // `[data-region="X"]` 選出每個區塊、以 grid-area 就位。
    topbar.setAttribute("data-region", "topbar");
    topbar.setAttribute("data-channel-state", latestChannelState);

    var name = el("span", "app-name", "AI Agent Cockpit");
    name.id = "app-name";
    topbar.appendChild(name);

    var lamps = el("div", "topbar-runtimes");
    for (var i = 0; i < state.runtimes.length; i += 1) {
      lamps.appendChild(renderRuntimeLamp(state.runtimes[i]));
    }
    topbar.appendChild(lamps);
    topbar.appendChild(renderNotifyBell());
    topbar.appendChild(renderLangToggle());

    return topbar;
  }

  // 通知鈴鐺（spec desktop-notifications「通知設定」；design D8；desktop-launch-notify task 3.3）：
  // 每次重畫都重建、開關狀態不存在 DOM 上（面板開著與否是 notify.js 的模組狀態）。帶
  // `data-action="notify-settings"`：焦點還原只認 `data-action` 元素，鍵盤停在鈴鐺上時重畫後焦點
  // 回到新的鈴鐺；actions.js 的 `perform()` 比照 select-project 提早處理，不算「畫面操作」。
  // 圖示是 currentColor 描邊的 SVG（顏色跟著按鈕文字色走，不引入 token 以外的顏色），可及名稱
  // 由 aria-label 提供。尺寸 14px＋1px 內距＋1px 框＝18px，不超過頂列行高（--topbar-line-height
  // × --fs-panel），不改變 --shell-topbar-h。
  var SVG_NS = "http://www.w3.org/2000/svg";

  function renderNotifyBell() {
    var bell = el("button", "notify-bell");
    bell.type = "button";
    bell.setAttribute("data-action", "notify-settings");
    bell.setAttribute("aria-label", t("render.notify.label"));
    bell.setAttribute("aria-controls", "notify-panel");
    // aria-expanded 依面板目前是否開著（修正波，3.5 採納項）：面板狀態在 notify.js，這裡每次重畫照實
    // 讀回，所以面板開著時重畫出來的新鈴鐺仍是 true；開關面板（不觸發重畫）時由 notify.js 直接改
    // 當下這顆鈴鐺的屬性。按下狀態的外觀由 style.css 依這個屬性呈現。
    var open =
      !!window.cockpitNotify &&
      typeof window.cockpitNotify.isOpen === "function" &&
      window.cockpitNotify.isOpen();
    bell.setAttribute("aria-expanded", open ? "true" : "false");
    bell.title = t("render.notify.label");

    var svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("viewBox", "0 0 16 16");
    svg.setAttribute("width", "14");
    svg.setAttribute("height", "14");
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("focusable", "false");
    var paths = [
      "M8 2.2c-2.2 0-3.8 1.7-3.8 3.9v2.6L2.8 11h10.4l-1.4-2.3V6.1C11.8 3.9 10.2 2.2 8 2.2z",
      "M6.4 12.6c.3.8.9 1.2 1.6 1.2s1.3-.4 1.6-1.2",
    ];
    for (var i = 0; i < paths.length; i += 1) {
      var path = document.createElementNS(SVG_NS, "path");
      path.setAttribute("d", paths[i]);
      path.setAttribute("fill", "none");
      path.setAttribute("stroke", "currentColor");
      path.setAttribute("stroke-width", "1.4");
      path.setAttribute("stroke-linejoin", "round");
      path.setAttribute("stroke-linecap", "round");
      svg.appendChild(path);
    }
    bell.appendChild(svg);
    return bell;
  }

  // 語言切換按鈕（spec ui-language「語言切換按鈕」；design D3）：鈴鐺旁，比照鈴鐺每次重畫都重建，
  // 帶 `data-action="toggle-language"`（焦點還原只認 data-action 元素）；按下由 actions.js 呼叫
  // `cockpitI18n.setLang(另一種語言)`＝寫 cockpit.lang 並重新載入頁面，所以這裡不存任何狀態。
  // 文字、lang 與可及名稱都是「按下後會變成的那種語言」的樣子，由字典提供（render.lang.*：繁中字典
  // 放 EN／en／Switch to English，英文字典放 中文／zh-Hant／切換為繁體中文——兩份值刻意各用目標語言寫，
  // 不隨介面語言翻譯）。localStorage 不可用（`cockpitI18n.canPersist` 為 false，載入時偵測一次）時
  // 選擇存不下來，按鈕停用並以 title 說明原因。
  function renderLangToggle() {
    var i18n = window.cockpitI18n;
    var button = el("button", "lang-toggle", i18n.t("render.lang.text"));
    button.type = "button";
    button.setAttribute("data-action", "toggle-language");
    button.setAttribute("lang", i18n.t("render.lang.code"));
    button.setAttribute("aria-label", i18n.t("render.lang.aria"));
    if (!i18n.canPersist) {
      button.disabled = true;
      button.title = i18n.t("render.lang.needsStorage");
    }
    return button;
  }

  // runtime 卡標題列右側的連線狀態：符號（跟頂列燈號、底列通道共用 .conn-symbol 的三種形狀，
  // design D4「連線」列）＋狀態文字，顏色依狀態（style.css .runtime-conn-*）。
  // ui-fixes task 4.3（spec cockpit-dashboard「畫面整頁重畫」通道斷線段落、design D2）：通道不是
  // connected 時，連線狀態文字前要標「最後已知」。比照頂列 .runtime-lamp-stale：一律輸出到 DOM、
  // 由 style.css 依 #app 的 data-channel-state 切換顯示，window.onChannel 不重畫就能生效；它是
  // .connection-state 的**手足**節點，.connection-state 的 textContent 維持精確等於連線狀態字串
  // （既有腳本以此精確比對）。
  function renderConnectionState(state) {
    var wrap = el("span", "runtime-conn " + connStateClass("runtime-conn-", state));
    var dot = el("span", "conn-symbol");
    dot.setAttribute("aria-hidden", "true");
    wrap.appendChild(dot);
    wrap.appendChild(el("span", "runtime-conn-stale", t("render.lamp.stale")));
    wrap.appendChild(el("span", "connection-state", state));
    return wrap;
  }

  // 連線明細（design D11；direction-01-visual task 3.3）：兩欄定義列表，左欄 --text-dim 標籤、
  // 右欄等寬值，一行一項，不用中點串接。狀態本身在卡片標題列（renderConnectionState），這裡
  // 只放明細；protocol 警告放在列表下方（警示色）。
  function renderConnection(runtime) {
    var connection = runtime.connection;
    var box = el("div", "connection conn-" + connection.state);
    var list = el("dl", "connection-details");
    function item(label, value) {
      list.appendChild(el("dt", null, label));
      list.appendChild(el("dd", null, orDash(value)));
    }
    item("endpoint", runtime.endpoint);
    if (connection.state === "connected") {
      item("server", connection.server_version);
      item("protocol", String(connection.protocol));
      item("last snapshot", connection.last_snapshot_at);
    } else if (connection.state === "disconnected") {
      item("reason", tMsg(connection.reason_msg, connection.reason));
      item("retry in", connection.retry_in_secs + "s");
    }
    box.appendChild(list);
    if (
      connection.state === "connected" &&
      connection.protocol_warning !== null &&
      connection.protocol_warning !== undefined
    ) {
      box.appendChild(
        el("div", "protocol-warning", tMsg(connection.protocol_warning_msg, connection.protocol_warning))
      );
    }
    return box;
  }

  // 操作按鈕：只帶 data-* 屬性，listener 由 actions.js 在 #app 根節點委派（design D9）。
  function actionButton(label, attrs) {
    var button = el("button", "action-button", label);
    button.type = "button";
    for (var key in attrs) {
      if (Object.prototype.hasOwnProperty.call(attrs, key)) {
        button.setAttribute("data-" + key, attrs[key]);
      }
    }
    return button;
  }

  function renderPane(pane, runtime, rebinding, selected) {
    var row = el("div", "pane-row");
    if (pane.exited) {
      row.classList.add("exited");
    }
    if (pane.focused) {
      row.classList.add("focused");
    }

    // data-runtime／data-pane 是結構標記（跟 ff-cell／ff-row-header 的 data-workstream 同樣
    // 性質：供測試腳本按 runtime＋pane 定位這一列，不是互動屬性），一律輸出、不受可不可以點選
    // 影響——否則改綁模式期間（R15 拿掉可點選屬性時）就無法用這兩個屬性定位到列了。
    row.setAttribute("data-runtime", runtime.id);
    row.setAttribute("data-pane", pane.id);

    // 被選定的標示跟「這一列現在可不可以點選」是兩件事（R15；spec live-output「選定一個
    // pane」：「既有的選取與面板在改綁模式期間保留」）：即使改綁模式期間這一列不可點選，若它
    // 就是目前的選取，`.selected` 仍要標示出來。
    if (selected !== null && selected.runtime === runtime.id && selected.paneId === pane.id) {
      row.classList.add("selected");
    }

    // 選取（spec live-output「選定一個 pane」）：exited 的 pane 不可選；改綁模式期間整列不可
    // 點選（R15：不呈現可點選樣式，點列的任何區域——「綁定到這裡」以外——都不改變選取；同時
    // 解掉 `.selectable:hover` 蓋掉 `.bind-target` 底色的樣式互蓋）。其餘一律可選——runtime
    // 未 connected 的 pane 也可選（spec 沒有限制；端點會回 503，面板如實呈現）。
    if (!pane.exited && !rebinding) {
      row.classList.add("selectable");
      row.setAttribute("data-action", "select-pane");
      // 鍵盤可及性：<button> 以外的可點元素要自己給 tabindex，Enter／Space 由 actions.js 的
      // keydown listener 補（瀏覽器不會替一般 <div tabindex> 自動轉成 click）。
      row.tabIndex = 0;
      row.setAttribute("role", "button");
    }

    // DOM 順序維持 id、agent、狀態、標題、cwd（design D11「CSS grid 定位、不改 DOM 順序」；
    // direction-01-visual task 3.3）：畫面上的兩行排列（第一行 狀態→agent→id 靠右，第二行
    // 標題→cwd）全部由 style.css 的 grid-template-areas 決定。標題與 cwd 單行省略，完整內容
    // 放在 title；id、agent 也可能很長，同樣帶 title。
    var idEl = el("span", "pane-id", pane.id);
    idEl.title = pane.id;
    row.appendChild(idEl);
    var agentName = pane.agent === null || pane.agent === undefined ? "shell" : pane.agent;
    var agentEl = el("span", "pane-agent", agentName);
    agentEl.title = agentName;
    row.appendChild(agentEl);
    row.appendChild(agentState(pane.agent_status));
    var titleEl = el("span", "pane-title", orDash(pane.title));
    if (pane.title !== null && pane.title !== undefined && pane.title !== "") {
      titleEl.title = pane.title;
    }
    row.appendChild(titleEl);
    // cwd 保留尾段（direction-01-visual task 3.3 fix round 1，設計審核 I1）：同一台機器上的路徑
    // 前綴幾乎都一樣，能區分 pane 的是最後一段。拆成前段（.pane-cwd-head，空間不夠時先省略）與
    // 尾段（.pane-cwd-tail，最後一段路徑，含前面的分隔符，完整可見）；兩段串起來仍是完整路徑，
    // 完整內容也放在 title。沒有前段（路徑裡沒有分隔符）時不產生空的前段 span——空 span 會讓
    // 第二行基線偏移、整列變高（direction-01-visual task 5.1，3.3 設計 N2）。
    var cwdEl;
    if (pane.cwd !== null && pane.cwd !== undefined && pane.cwd !== "") {
      var cwdParts = splitPathTail(pane.cwd);
      cwdEl = el("span", "pane-cwd");
      if (cwdParts[0] !== "") {
        cwdEl.appendChild(el("span", "pane-cwd-head", cwdParts[0]));
      }
      cwdEl.appendChild(el("span", "pane-cwd-tail", cwdParts[1]));
      cwdEl.title = pane.cwd;
    } else {
      cwdEl = el("span", "pane-cwd", orDash(pane.cwd));
    }
    row.appendChild(cwdEl);

    // HERDR 目前聚焦的 pane（task 3.3 fix round 1，設計審核 M3）：不用底色（跟滑過、改綁目標
    // 幾乎同色，也沒有說明），改用一個文字標記，放在 DOM 文字裡、不用 CSS content。`.focused`
    // class 照舊保留。
    if (pane.focused) {
      var focusMark = el("span", "pane-herdr-focus", t("render.pane.focus"));
      focusMark.title = t("render.pane.focusTitle");
      row.appendChild(focusMark);
    }

    // 改綁模式：connected runtime 中未 exited 的 pane 列才出現「綁定到這裡」（spec「畫面操作」）。
    if (rebinding && runtime.connection.state === "connected" && !pane.exited) {
      row.classList.add("bind-target");
      row.appendChild(
        actionButton(t("render.pane.bindHere"), { action: "bind-here", runtime: runtime.id, pane: pane.id })
      );
    }

    return row;
  }

  function renderTab(tab, runtime, rebinding, selected) {
    var box = el("div", "tab");
    if (tab.focused) {
      box.classList.add("focused");
    }

    var header = el("div", "tab-header");
    header.appendChild(el("span", "tab-number", "tab " + tab.number));
    header.appendChild(agentState(tab.agent_status));
    box.appendChild(header);

    for (var i = 0; i < tab.panes.length; i += 1) {
      box.appendChild(renderPane(tab.panes[i], runtime, rebinding, selected));
    }

    return box;
  }

  function renderWorkspace(workspace, runtime, rebinding, selected) {
    var box = el("div", "workspace");
    if (workspace.focused) {
      box.classList.add("focused");
    }

    // 一行標題列（design D11）：label（過長時單行省略，完整在 title）、#number、彙總狀態。
    var header = el("div", "workspace-header");
    var labelText = workspace.label === null || workspace.label === undefined ? workspace.id : workspace.label;
    var labelEl = el("span", "workspace-label", labelText);
    labelEl.title = labelText;
    header.appendChild(labelEl);
    header.appendChild(el("span", "workspace-number", "#" + workspace.number));
    header.appendChild(agentState(workspace.agent_status));
    box.appendChild(header);

    for (var i = 0; i < workspace.tabs.length; i += 1) {
      box.appendChild(renderTab(workspace.tabs[i], runtime, rebinding, selected));
    }

    return box;
  }

  // Factory Floor（spec cockpit-dashboard「Factory Floor」）：每個 Project 一塊，欄為
  // stages、列為 workstreams，交會格放這個 workstream 在目前 stage 的 task 節點。

  var KNOWN_TASK_STATUSES = ["running", "blocked", "ready", "pending", "failed", "completed"];

  function taskStatusClass(status) {
    return KNOWN_TASK_STATUSES.indexOf(status) !== -1
      ? "task-status-" + status
      : "task-status-unknown";
  }

  // 左欄計數 chip（spec cockpit-dashboard「Project 切換」；design D11「左欄計數 chip」；
  // direction-01-visual task 3.1）：固定依「需要注意」的程度排序，數量為 0 的不顯示。這裡跟
  // `KNOWN_TASK_STATUSES`（Factory Floor 節點用，running 優先）故意分開放一份獨立順序——
  // 兩者服務不同的排序語意（節點的「已知狀態集合」vs 計數的「注意力排序」），共用一份會讓其中
  // 一邊的順序變動意外影響另一邊。
  var TASK_STATUS_COUNT_ORDER = [
    "failed",
    "blocked",
    "running",
    "ready",
    "pending",
    "completed",
  ];

  // design D4「對象／狀態→顏色與符號」task 列：pending ○、ready ♢（U+2662；direction-01-visual
  // task 3.2 由 ◇ 換掉——◇ 在 Segoe UI 12px 下墨跡只有約 7×6px，其他符號約 10×9px，看起來
  // 小一半，見 design D4「狀態符號」）、running ▶︎（U+25B6 接
  // U+FE0E 文字呈現選擇字元，避免被部分字型畫成彩色 emoji）、blocked ‖、failed ✕、
  // completed ✓。左欄計數 chip（task 3.1）與 Factory Floor 節點（direction-01-visual task
  // 3.2）共用這一份對照，兩處的符號因此不會各自漂移。
  var TASK_STATUS_SYMBOLS = {
    pending: "○",
    ready: "♢",
    running: "▶︎",
    blocked: "‖",
    failed: "✕",
    completed: "✓",
  };
  var UNKNOWN_STATUS_SYMBOL = "?";

  function taskStatusSymbol(status) {
    return Object.prototype.hasOwnProperty.call(TASK_STATUS_SYMBOLS, status)
      ? TASK_STATUS_SYMBOLS[status]
      : UNKNOWN_STATUS_SYMBOL;
  }

  function taskStatusCountClass(status) {
    return TASK_STATUS_COUNT_ORDER.indexOf(status) !== -1
      ? "project-count-" + status
      : "project-count-unknown";
  }

  // 依 project.tasks 逐一累計每個 status 的數量，回傳「依 D11 排序、數量 > 0」的清單
  // （[{status, count}, ...]）。已知的六種 status 固定在前、依 TASK_STATUS_COUNT_ORDER 排序；
  // 任何不在這六種之內的字串（未來協定加的新值——StageStatus 目前是封閉 enum，正常情況不會
  // 發生，這裡沿用整份檔案「未知不破壞畫面」的慣例）各自成一個 chip，接在已知六種之後、依第一次
  // 出現的順序排列，是 D11「未知」這個順位的具體實作（同一批未知字串裡有兩種以上時全部列出、
  // 不合併成一個模糊的「未知」，跟 Factory Floor 節點「未知狀態顯示原字串」的原則一致）。
  function computeTaskStatusCounts(project) {
    var counts = {};
    var unknownOrder = [];
    for (var i = 0; i < project.tasks.length; i += 1) {
      var status = project.tasks[i].status;
      if (counts[status] === undefined) {
        counts[status] = 0;
        if (TASK_STATUS_COUNT_ORDER.indexOf(status) === -1) {
          unknownOrder.push(status);
        }
      }
      counts[status] += 1;
    }
    var order = TASK_STATUS_COUNT_ORDER.concat(unknownOrder);
    var result = [];
    for (var j = 0; j < order.length; j += 1) {
      var s = order[j];
      if (counts[s] > 0) {
        result.push({ status: s, count: counts[s] });
      }
    }
    return result;
  }

  // 一個計數 chip＝符號（aria-hidden）＋status 文字＋數字，三者是各自獨立的 span（design D11
  // 「不用中點串接」——串接會變成單一字串，這裡刻意留三個子節點，CSS 用 gap 分隔，跟「數量為 0
  // 的不顯示」一起讓「有 failed／blocked 時第一格就看得到暖色」在 DOM 結構上就成立：chip 本身
  // 的顏色（見 style.css .project-count-*）由 D4 對照表決定，不用冰青搶份量）。
  function renderStatusCountChip(entry) {
    var cls = taskStatusCountClass(entry.status);
    var chip = el("span", "project-count " + cls);
    var symbol = el("span", "project-count-symbol", taskStatusSymbol(entry.status));
    symbol.setAttribute("aria-hidden", "true");
    chip.appendChild(symbol);
    chip.appendChild(el("span", "project-count-status", entry.status));
    chip.appendChild(el("span", "project-count-number", String(entry.count)));
    if (cls === "project-count-unknown") {
      chip.title = entry.status;
    }
    return chip;
  }

  // warnings 數量（design D11「warnings 用 --warn，放在名稱同一行右側」）：文字含「警告」二字
  // （不是符號單獨表達），滿足「凡是以顏色表達的狀態都必須同時以文字呈現」的一般原則，也讓
  // 「有 N 則 warning」這件事本身可以只讀文字判斷、不必只靠顏色。
  function renderWarningCount(count) {
    return el("span", "project-item-warnings", tn("render.warning.count", count));
  }

  // 非 bound 狀態的單段文字（.ff-binding-text 單行省略，style.css）：英文比繁中長，欄寬不夠時會被截斷，
  // 全文同時放在外層 .ff-binding 的 title（同一個字典字串），滑鼠停在上面看得到。
  function appendBindingText(wrap, text) {
    wrap.title = text;
    wrap.appendChild(el("span", "ff-binding-text", text));
  }

  function renderBindingSummary(binding) {
    var wrap = el("span", "ff-binding ff-binding-" + binding.state);
    switch (binding.state) {
      case "bound":
        // fix round 4／N3（direction-01-visual task 2.1）：runtime／pane ID 可能很長、沒有斷行點，
        // 全文放在外層 .ff-binding 的 title（滑鼠停在文字上一樣看得到）。
        // direction-01-visual task 3.2（2.1 設計審核 r4 Minor）：原本整串「runtime / pane」是
        // 同一個單行省略的 span，runtime 名稱一長，省略號就把 pane id 吃掉。拆成 runtime、分隔、
        // pane 三個 span：只有 runtime 單行省略；pane id 不省略，放不下時整段換到下一行、太長才
        // 在任意字元斷行，永遠看得到（style.css .ff-binding-*）。.ff-binding-text 的
        // textContent 仍然是「runtime / pane」全文，腳本以 textContent 比對。
        wrap.title = binding.runtime + " / " + binding.pane_id;
        var boundText = el("span", "ff-binding-text");
        boundText.appendChild(el("span", "ff-binding-runtime", binding.runtime));
        boundText.appendChild(el("span", "ff-binding-sep", " / "));
        boundText.appendChild(el("span", "ff-binding-pane", binding.pane_id));
        wrap.appendChild(boundText);
        if (binding.source === "override") {
          wrap.appendChild(el("span", "ff-binding-badge", t("render.binding.rebound")));
        }
        break;
      case "unbound":
        appendBindingText(wrap, t("render.binding.unbound"));
        break;
      case "ambiguous":
        appendBindingText(wrap, tn("render.binding.ambiguous", binding.candidates.length));
        break;
      case "runtime_disconnected":
        appendBindingText(wrap, t("render.binding.runtimeDisconnected"));
        // ui-fixes task 4.4（spec「Factory Floor」；design D3）：覆蓋造成的斷線（source 為
        // override）同樣顯示「改綁」徽章，自動綁定的斷線（auto）沒有；徽章樣式沿用 bound 的。
        if (binding.source === "override") {
          wrap.appendChild(el("span", "ff-binding-badge", t("render.binding.rebound")));
        }
        break;
      case "none":
        appendBindingText(wrap, t("render.binding.none"));
        break;
      default:
        // 防禦：未知的 binding.state 不該發生（投影只會輸出這五種之一），但沿用整份檔案
        // 「未知值不壞畫面」的原則，顯示原字串而不是丟例外。
        appendBindingText(wrap, String(binding.state));
    }
    return wrap;
  }

  // 節點按鈕顯示規則（spec「畫面操作」）：mark 為 none 且不在第一個 stage →「退回」（放在
  // 「推進」之前，progress-model task 4.2）；mark 為 none 且不在最後一個 stage →「推進」；
  // mark 為 none →「Completed」「Failed」；mark 不是 none → 只有「清除標記」。
  function renderTaskActions(project, task) {
    var actions = el("div", "task-actions");
    function add(label, action) {
      actions.appendChild(
        actionButton(label, { action: action, project: project.id, task: task.id })
      );
    }
    if (task.mark === "none") {
      if (task.stage !== project.stages[0]) {
        add(t("render.task.back"), "retreat");
      }
      if (task.stage !== project.stages[project.stages.length - 1]) {
        add(t("render.task.advance"), "advance");
      }
      add("Completed", "complete");
      add("Failed", "fail");
    } else {
      add(t("render.task.clearMark"), "clear");
    }
    return actions;
  }

  // 卡片的 OpenSpec 同步標示（spec cockpit-dashboard「卡片的 OpenSpec 同步標示」；design D9；
  // openspec-stage-sync task 5.1）：task 的 sync 不是 null 才多一行「change 名稱 checked/total 自動／手動」，
  // 位於「符號＋status」與按鈕之間；sync 為 null（或欄位缺漏）回傳 null、節點與沒有此功能時完全相同。
  // change 名稱是使用者資料，一律 el() 的文字節點（textContent），不以 HTML 插入；整行是唯讀文字，沒有互動。
  // 手動較淡由 style.css 的 [data-sync-mode="manual"] 決定，文字本身（自動／手動）才是主要區分。
  function renderTaskSync(sync) {
    if (!sync || typeof sync !== "object") return null;
    var mode = sync.mode;
    var modeText =
      mode === "auto" ? t("render.task.syncAuto") : mode === "manual" ? t("render.task.syncManual") : String(mode);
    var row = el("div", "task-sync");
    // 未知 mode 不壞畫面：照原字串顯示，樣式落在預設（自動）那一套。
    row.setAttribute("data-sync-mode", mode === "manual" ? "manual" : "auto");
    row.title = t("render.task.syncTitle", {
      change: sync.change,
      checked: sync.checked,
      total: sync.total,
      mode: modeText,
    });
    row.appendChild(el("span", "task-sync-change", String(sync.change)));
    row.appendChild(el("span", "task-sync-progress", String(sync.checked) + "/" + String(sync.total)));
    row.appendChild(el("span", "task-sync-mode", modeText));
    return row;
  }

  // Task 節點（spec「Factory Floor」；design D4；direction-01-visual task 3.2）：由上往下固定
  // 是「標題 → 符號＋status 文字 → 同步標示（task 有 sync 才有，renderTaskSync()）→ 按鈕」，沒有 sync 時是三段、有時是
  // 四段（openspec-stage-sync task 5.1）。符號是 aria-hidden 的 span（design D4「符號
  // 放在 DOM 文字裡，不用 CSS content」），跟左欄計數 chip 共用 taskStatusSymbol()；status
  // 文字沿用投影的英文原字串（不翻成中文，http.rs 的禁字測試守著）。色條、外框、柔光全部由 style.css 依
  // .task-status-* 決定，這裡只負責結構。
  function renderTaskNode(project, task) {
    var cls = taskStatusClass(task.status);
    var node = el("div", "task-node " + cls);
    if (cls === "task-status-unknown") {
      node.title = task.status;
    }
    var titleEl = el("span", "task-title", task.title);
    titleEl.title = task.title; // 最多兩行、超過省略，完整標題放 title（design D3）
    node.appendChild(titleEl);

    var state = el("span", "task-state");
    var symbol = el("span", "task-status-symbol", taskStatusSymbol(task.status));
    symbol.setAttribute("aria-hidden", "true");
    state.appendChild(symbol);
    state.appendChild(el("span", "task-status-label", task.status));
    node.appendChild(state);

    var syncRow = renderTaskSync(task.sync);
    if (syncRow) node.appendChild(syncRow);

    node.appendChild(renderTaskActions(project, task));
    return node;
  }

  function renderWorkstreamRowHeader(project, workstream) {
    var header = el("div", "ff-row-header");
    // 跟 ff-cell 一樣的 data-workstream 只是結構標記（供測試腳本按 id 定位這一列，兩個不同
    // Project 可能有同名 workstream.name，光用文字找不準），不是互動屬性。
    header.setAttribute("data-workstream", workstream.id);
    var wsNameEl = el("span", "ff-ws-name", workstream.name);
    wsNameEl.title = workstream.name; // 長字串換行顯示（design D3；direction-01-visual task 2.1）
    header.appendChild(wsNameEl);
    // Repo Project 工作線的 worktree 標註（spec「Repo Project 工作線的 worktree 標註」；repo-projects
    // task 5.3）：直接用投影的 `worktree` 欄位（linked worktree 的資料夾名稱，前端不複算），主 worktree
    // 的工作線沒有該欄位（或空字串）就不畫。一律 textContent（el() 的文字節點），不以 HTML 插入。
    if (typeof workstream.worktree === "string" && workstream.worktree !== "") {
      var worktreeEl = el("span", "ff-worktree", workstream.worktree);
      worktreeEl.title = t("render.row.worktree", { name: workstream.worktree });
      header.appendChild(worktreeEl);
    }
    header.appendChild(renderBindingSummary(workstream.binding));
    // 「工作中・未宣告 task」（spec「Factory Floor」；progress-model task 4.3、design D7）：
    // 完全由投影的 activity_undeclared 決定（整頁重畫不丟狀態）；只有 true 才畫，其他值
    // （false、欄位缺漏）一律不顯示。警示色靜態文字，沒有動畫（style.css .ff-undeclared）。
    if (workstream.activity_undeclared === true) {
      header.appendChild(el("span", "ff-undeclared", t("render.row.undeclared")));
    }

    // 列首操作：「改綁」除了 binding.source 為 pane（Repo Project 固定 pane 的工作線，覆蓋端點會回
    // 409 not_overridable）之外都有；binding.source 為 override 時另有「取消改綁」（spec「畫面操作」；
    // repo-projects task 5.3）。source 有 auto／override／pane 三個值：這裡只排除 pane，其餘維持舊行為
    // （auto、未知值照舊顯示「改綁」）；「取消改綁」與「改綁」徽章是 === "override"，pane 自然不在內。
    var actions = el("div", "ff-row-actions");
    // 「看輸出」只在 binding 為 bound 時出現（spec live-output「選定一個 pane」）：按下選定它
    // 綁定的那個 runtime＋pane。
    //
    // fix round 1 Finding 2：兩個 workstream 綁到同一個 pane 時，若身分只看 action＋runtime＋
    // pane，兩顆「看輸出」的身分會完全相同——焦點還原（見下方 captureFocusIdentity 等）遇到
    // 重複身分取文件順序第一個，焦點原本在後一列時重畫後會錯誤跳到第一列。修法：另外帶
    // `project` 與 `source-workstream`（→ `dataset.sourceWorkstream`）讓每顆「看輸出」的身分
    // 在同一個 project 內唯一（同一個 pane 不會被同一個 project 的同一個 workstream 綁兩次）。
    // **刻意不用 `workstream` 這個屬性名**（也就是不輸出 `data-workstream`）：
    // `actions-check.js` 既有的「按鈕顯示規則」斷言用 `[data-action][data-workstream]` 選出
    // 每個 workstream 列首「應該有哪些操作」，若「看輸出」也帶 `data-workstream`，它會被算進
    // 那個集合、跟預期的 `['rebind']`／`['override-clear','rebind']` 兜不起來，使既有斷言
    // 失敗（brief「不得改它們的斷言」）。`select-pane`（pane 列本身）與 `bind-here`（綁定到
    // 這裡）都以 runtime＋pane 就能唯一指認同一列（同一個 runtime＋pane 只會出現在唯一一列
    // pane 上），不受這個碰撞影響，不需要同樣的處理；重新盤點過 `render.js` 其餘帶
    // `data-action` 的元素（task 按鈕靠 project＋task、workstream 列首「改綁」／「取消改綁」
    // 靠 project＋workstream、改綁提示「取消」與錯誤訊息「關閉」全域唯一一個），沒有發現其他
    // 碰撞。
    if (workstream.binding.state === "bound") {
      actions.appendChild(
        actionButton(t("render.row.viewOutput"), {
          action: "select-bound-pane",
          runtime: workstream.binding.runtime,
          pane: workstream.binding.pane_id,
          project: project.id,
          "source-workstream": workstream.id,
        })
      );
    }
    if (workstream.binding.source !== "pane") {
      actions.appendChild(
        actionButton(t("render.row.rebind"), { action: "rebind", project: project.id, workstream: workstream.id })
      );
    }
    if (workstream.binding.source === "override") {
      actions.appendChild(
        actionButton(t("render.row.undoRebind"), {
          action: "override-clear",
          project: project.id,
          workstream: workstream.id,
        })
      );
    }
    header.appendChild(actions);
    return header;
  }

  function renderFactoryCell(project, workstream, stageName) {
    var cell = el("div", "ff-cell");
    cell.setAttribute("data-workstream", workstream.id);
    cell.setAttribute("data-stage", stageName);

    // 依設定順序（= project.tasks 的既有順序）排列同格多個 task；不畫依賴箭頭。
    for (var i = 0; i < project.tasks.length; i += 1) {
      var task = project.tasks[i];
      if (task.workstream === workstream.id && task.stage === stageName) {
        cell.appendChild(renderTaskNode(project, task));
      }
    }

    return cell;
  }

  // 網格軌道下限（px）：renderFactoryFloor() 的 gridTemplateColumns 與 renderProject() 的面板
  // min-width 共用這兩個數字，改一處就兩處一起變（fix round 4／N3 的算術下限，見
  // renderProject()）。direction-01-visual task 3.2：網格改成 gap: 0（見 style.css
  // .factory-floor——sticky 列首／欄首之間不能留縫，否則捲過去的節點會從縫裡透出來），間距改由
  // 格子自己的 padding 負責，算式因此不再加 gap。
  // task 3.2 fix round 1（設計審核 M1）：首欄上限 240 → 200。grid 會先把有固定上限的軌道撐到
  // 上限、剩下的才分給 1fr，1100–1280 寬時列首內容只有約 170px 卻吃滿 240，stage 欄只剩約
  // 140px，節點三顆按鈕疊成三列；降到 200 後 1280×650（含內層捲軸）stage 欄約 154px，搭配
  // style.css 格子與節點左右內距 8 → 6px，按鈕變兩列。上限同時是
  // .projects 的 scroll-padding-left（renderState()），兩處共用這個常數。面板 min-width 的算術
  // 只用下限（renderProject()），不受上限影響。
  var FF_ROW_HEADER_MIN_PX = 160;
  var FF_ROW_HEADER_MAX_PX = 200;
  var FF_STAGE_MIN_PX = 140;
  // 替鍵盤焦點框留的捲動餘裕：外推 4px（style.css :focus-visible 的 outline-offset 2px＋
  // outline 寬 2px）＋2px 取整餘裕（捲動位置是整數像素、容器邊可能落在小數位置，實測差 0.5px）。
  var FOCUS_RING_ROOM_PX = 6;

  function stageHasRunningTask(project, stageName) {
    for (var i = 0; i < project.tasks.length; i += 1) {
      if (project.tasks[i].stage === stageName && project.tasks[i].status === "running") {
        return true;
      }
    }
    return false;
  }

  // stage 欄首（design D3／D8；direction-01-visual task 3.2）：上緣一格刻度（DOM 元素，不是偽
  // 元素），跟著欄一起橫向捲動、永遠跟下方欄位對齊；這個 stage 有 running task 時加
  // .ff-stage-running，style.css 把該格刻度畫得較長、改用 --text（不用冰青，D4「冰青的形狀
  // 分工」）。data-stage 只是結構標記（供腳本把刻度對回欄位），不是互動屬性。
  function renderStageHeader(project, stageName) {
    var running = stageHasRunningTask(project, stageName);
    var header = el("div", "ff-stage-header" + (running ? " ff-stage-running" : ""));
    header.setAttribute("data-stage", stageName);
    var tick = el("span", "ff-stage-tick");
    tick.setAttribute("aria-hidden", "true");
    header.appendChild(tick);
    var name = el("span", "ff-stage-name", stageName);
    name.title = stageName;
    header.appendChild(name);
    return header;
  }

  function renderFactoryFloor(project) {
    var grid = el("div", "factory-floor");
    // 欄數隨這個 Project 的 stages 數量而定，交由 JS 算出（style.css 只定義固定樣式）。
    // fix round 4／N3（Codex r3）：首欄（workstream 列首）有上限（task 3.2 fix round 1 起 200px，見 FF_ROW_HEADER_MAX_PX）——原本是 minmax(160px, auto)，
    // auto 上限會被列首內容（長 runtime／pane ID）撐寬，吃掉 stage 欄的空間；現在列首內容
    // 自己換行（名稱）或單行省略（binding 文字），首欄寬度落在 [下限, 上限]。
    grid.style.gridTemplateColumns =
      "minmax(" + FF_ROW_HEADER_MIN_PX + "px, " + FF_ROW_HEADER_MAX_PX + "px) repeat(" +
      project.stages.length + ", minmax(" + FF_STAGE_MIN_PX + "px, 1fr))";

    grid.appendChild(el("div", "ff-corner"));
    for (var s = 0; s < project.stages.length; s += 1) {
      grid.appendChild(renderStageHeader(project, project.stages[s]));
    }

    for (var w = 0; w < project.workstreams.length; w += 1) {
      var workstream = project.workstreams[w];
      grid.appendChild(renderWorkstreamRowHeader(project, workstream));
      for (var s2 = 0; s2 < project.stages.length; s2 += 1) {
        grid.appendChild(renderFactoryCell(project, workstream, project.stages[s2]));
      }
    }

    return grid;
  }

  // Factory Floor 標題列（spec「Factory Floor」「標題為 Project name，有 warnings 時逐則顯示」；
  // design D10 主標題 20/600；direction-01-visual task 3.2）：放在不捲動的外框
  // （data-region="floor"）裡、內層捲動容器 .projects 之上——網格橫向或縱向捲動時標題與
  // warnings 都不會被捲走。標題列不放刻度（design D8：刻度改在 stage 欄首上緣）。
  function renderFloorTitlebar(project) {
    var header = el("div", "project-header");
    var nameEl = el("h2", "project-name", project.name);
    nameEl.title = project.name; // 單行省略，完整名稱放 title（design D3）
    header.appendChild(nameEl);
    if (project.warnings.length > 0) {
      var warnings = el("ul", "project-warnings");
      for (var i = 0; i < project.warnings.length; i += 1) {
        // warning_msgs 與 warnings 等長同順序；舊投影沒有這個欄位或比較短時，該筆退回原文。
        var warningMsg = Array.isArray(project.warning_msgs) ? project.warning_msgs[i] : null;
        warnings.appendChild(el("li", "project-warning", tMsg(warningMsg, project.warnings[i])));
      }
      header.appendChild(warnings);
    }
    return header;
  }

  function renderProject(project) {
    var section = el("div", "project");
    section.setAttribute("data-project", project.id);

    section.appendChild(renderFactoryFloor(project));

    // fix round 3／N3（設計複審 r2 新找到的退步，2.1 修法改用 JS 算 min-width，不是 CSS
    // `width: max-content`）：N3 原本的問題是「網格橫向溢出改由 `.projects` 承接後，`.project`
    // 面板自己的框沒有跟著網格變寬」。第一版修法在 style.css 給 `.project` 加
    // `width: max-content`，複審（round 3 自查）發現這會連帶把面板寬度撐得比實際需要的還寬：
    // CSS Flexbox 規格（§9.9）規定「計算 flex-wrap: wrap 容器的 max-content 尺寸時，视同
    // flex-wrap: nowrap（所有項目擠在同一行）」——`.ff-row-actions`／`.task-actions` 都是
    // `flex-wrap: wrap`，一旦 `.project` 改問「我的 max-content 是多少」，這兩種按鈕列都會照
    // 「全部擠一行」回報寬度，`.factory-floor` 的 grid 欄位（含 `minmax(140px, 1fr)` 的 stage
    // 欄——瀏覽器在算 grid 容器自身 intrinsic size 時，`fr` 的 max track sizing function 會
    // 降級當 `auto` 處理）也跟著抓到這個灌水後的寬度，即使 stage 數很少的預設投影也會被撐寬到
    // 超出可視範圍，讓某些按鈕的可見殘影卡在面板裁切邊界上、疊到 `.projects` 自己的
    // （overlay）捲軸，點不到本人（live-output-check.js E 段命中測試抓到）。
    // 改法：不問瀏覽器「max-content 是多少」，直接用跟 `renderFactoryFloor()` 設
    // `gridTemplateColumns` 同一組數字（`minmax(160px, 200px)` 的列首欄、`minmax(140px, 1fr)`
    // 的 stage 欄；task 3.2 起 gap 為 0）算出網格的最小需要寬度，用 `min-width` 當下限（不是
    // `width`／`max-content`，不觸發瀏覽器對子樹做 intrinsic-size 查詢，`.ff-row-actions`／
    // `.task-actions` 照正常版面演算法算，該怎麼換行就怎麼換行）。`max(100%, …px)`：容器夠寬
    // 時跟原本一樣填滿 100%；stage 數多到超過容器時至少撐到網格需要的寬度，`.projects` 就能
    // 橫向捲動看到完整的面板框線與內容（跟原本 N3 想要的效果一致）。
    // fix round 4／N3（Codex r3 質疑「把首欄當 160」）：首欄軌道的下限是固定值 160（不是 auto），
    // grid 的 base size 就是 160、不受列首內容影響；首欄只在容器有剩餘空間時才長向上限（200），
    // stage 欄也不會因此低於 140，所以網格寬度恆為 max(容器寬, 下面的算術值)，這個下限成立。
    // 實測（長 runtime／pane ID、10 stage）：網格 scrollWidth＝clientWidth＝1620＝算術值。
    // direction-01-visual task 3.2：網格 gap 改為 0、`.project` 不再畫框（2.2 設計審核 F7「框中
    // 框」：外框已經是 data-region="floor"，裡面的 .project 不再有 padding／border），面板下限
    // 因此就是網格下限本身，不再加 gap 與框的寬度。
    var stageCount = project.stages.length;
    var panelMinWidthPx = FF_ROW_HEADER_MIN_PX + stageCount * FF_STAGE_MIN_PX;
    section.style.minWidth = "max(100%, " + panelMinWidthPx + "px)";

    return section;
  }

  function renderRuntimeCard(runtime, rebinding, selected) {
    // design D11（direction-01-visual task 3.3）：右欄只有 runtime 卡本身一層框；卡內
    // workspace／tab 用分隔線與縮排表現層次（見 style.css）。標題列＝runtime id（等寬、過長時
    // 單行省略，完整在 title）＋連線狀態；endpoint 移進連線明細的定義列表。
    var card = el("div", "runtime-card");

    var header = el("div", "runtime-header");
    var idEl = el("span", "runtime-id", runtime.id);
    idEl.title = runtime.id;
    header.appendChild(idEl);
    header.appendChild(renderConnectionState(runtime.connection.state));
    card.appendChild(header);

    card.appendChild(renderConnection(runtime));

    for (var i = 0; i < runtime.workspaces.length; i += 1) {
      card.appendChild(renderWorkspace(runtime.workspaces[i], runtime, rebinding, selected));
    }

    return card;
  }

  function eventSubject(event) {
    if (event.pane_id) {
      return event.pane_id;
    }
    if (event.tab_id) {
      return event.tab_id;
    }
    if (event.workspace_id) {
      return event.workspace_id;
    }
    return "";
  }

  // 把路徑拆成 [前段, 尾段]：尾段＝最後一個分隔符（\ 或 /）起到結尾；結尾的分隔符算在尾段裡。
  // 沒有分隔符（或只在開頭）時整串都是尾段。
  function splitPathTail(path) {
    var end = path.length;
    while (end > 0 && (path.charAt(end - 1) === "\\" || path.charAt(end - 1) === "/")) {
      end -= 1;
    }
    var cut = end > 0 ? Math.max(path.lastIndexOf("\\", end - 1), path.lastIndexOf("/", end - 1)) : -1;
    if (cut <= 0) {
      return ["", path];
    }
    return [path.slice(0, cut), path.slice(cut)];
  }

  function eventTimeOfDay(at) {
    if (typeof at !== "string") {
      return orDash(at);
    }
    var tIdx = at.indexOf("T");
    return tIdx === -1 ? at : at.slice(tIdx + 1);
  }

  function renderRecentEvents(events) {
    var section = el("div", "recent-events");
    section.setAttribute("data-region", "events");
    section.appendChild(el("h2", "recent-events-title", t("index.events.title")));

    var list = el("ul", "recent-events-list");
    var limited = events.slice(0, 50);
    for (var i = 0; i < limited.length; i += 1) {
      var event = limited[i];
      var item = el("li", "event-row");
      // design D11（direction-01-visual task 3.3）：at 只顯示時間部分（例如 01:59:30Z，保留
      // Z、不轉時區），完整字串放 title；不是「日期T時間」格式時原樣顯示。
      var atEl = el("span", "event-at", eventTimeOfDay(event.at));
      atEl.title = orDash(event.at);
      item.appendChild(atEl);
      item.appendChild(el("span", "event-runtime", event.runtime));
      item.appendChild(el("span", "event-kind", event.kind));
      item.appendChild(el("span", "event-subject", eventSubject(event)));
      item.appendChild(el("span", "event-detail", tMsg(event.detail_msg, event.detail)));
      list.appendChild(item);
    }
    section.appendChild(list);

    return section;
  }

  // 改綁模式提示：指出目標 workstream（找得到就用 Project／workstream 的 name；找不到——例如
  // 新投影裡那條 workstream 已被移除——退回顯示 id）＋「取消」。
  function renderRebindBanner(state, rebind) {
    var projectName = rebind.project;
    var workstreamName = rebind.workstream;
    for (var p = 0; p < state.projects.length; p += 1) {
      var project = state.projects[p];
      if (project.id !== rebind.project) {
        continue;
      }
      projectName = project.name;
      for (var w = 0; w < project.workstreams.length; w += 1) {
        if (project.workstreams[w].id === rebind.workstream) {
          workstreamName = project.workstreams[w].name;
        }
      }
    }
    var banner = el("div", "action-banner rebind-banner");
    banner.appendChild(
      el(
        "span",
        "action-banner-text",
        t("render.rebind.banner", {
          project: projectName,
          workstream: workstreamName,
          bindHere: t("render.pane.bindHere"),
        })
      )
    );
    banner.appendChild(actionButton(t("render.rebind.cancel"), { action: "rebind-cancel" }));
    return banner;
  }

  // direction-01-visual task 3.4 fix round 1（設計 M2）：灰階瞇眼圖下，錯誤與改綁提示原本只靠
  // 色相與文字內容區分——補一個 aria-hidden 的 ✕ 符號（跟 Factory Floor failed 節點同一個字元，
  // 見 TASK_STATUS_SYMBOLS.failed），朗讀輔助不受影響（role="alert" 唸的是後面的訊息文字）。
  // 改綁提示不加符號（M2 只點名錯誤提示）。
  function renderErrorBanner(message) {
    var banner = el("div", "action-banner error-banner");
    banner.setAttribute("role", "alert");
    var symbol = el("span", "action-banner-symbol", TASK_STATUS_SYMBOLS.failed);
    symbol.setAttribute("aria-hidden", "true");
    banner.appendChild(symbol);
    banner.appendChild(el("span", "action-banner-text", message));
    banner.appendChild(actionButton(t("render.error.close"), { action: "error-dismiss" }));
    return banner;
  }

  // 錯誤／改綁提示合併成單一 data-region="banner"（design D2；direction-01-visual task
  // 2.1）：兩者都不存在時整個不輸出——`.shell` 的 grid-template-areas 裡「banner」那一列因此
  // 沒有任何內容撐開，auto 高度收成 0，不需要另外用 CSS 條件式隱藏。回傳 null 代表「這次不畫
  // 這個區塊」，呼叫端自己判斷要不要 appendChild。
  function renderBannerRegion(state, error, rebind) {
    if (error === null && rebind === null) {
      return null;
    }
    var region = el("div", "region-banner");
    region.setAttribute("data-region", "banner");
    if (error !== null) {
      region.appendChild(renderErrorBanner(error));
    }
    if (rebind !== null) {
      region.appendChild(renderRebindBanner(state, rebind));
    }
    return region;
  }

  // 沒有 Project 的空狀態文案（design D11 逐字文案；direction-01-visual task 3.1／fix round 1
  // M4）：D11 只給了 Factory Floor 那則的逐字文案，指向動作的完整說明（改 cockpit.toml、加
  // [[project]]、需要重啟）留給那一則；左欄這則 fix round 1 之前重複了幾乎一樣的句子（兩段
  // 上下或左右相鄰時讀起來像同一句話說兩次，「在這裡看到」也沒有受詞），設計審核 M4 建議改成
  // 只講狀態，這裡採用：左欄只寫「沒有 Project」，不再重複 Factory Floor 那句的說明。
  // （字串在字典 render.projects.empty；繁中值逐字不變。）
  // Factory Floor 那則（字典 render.floor.empty）在 repo-projects task 5.1 改寫：spec「Project 切換」要求
  // 空狀態指向左欄「偵測到的 repo」區（在那裡加入 repo 即可看到 Factory Floor），不得出現需要重啟的字樣
  // ——加入 Repo Project 不需要重啟，原本 D11「改 cockpit.toml、需要重啟」的說明已不是主要路徑。

  function renderProjectsEmptyState() {
    return el("div", "projects-empty-state", t("render.projects.empty"));
  }

  function renderFloorEmptyState() {
    return el("div", "floor-empty-state", t("render.floor.empty"));
  }

  // 左欄一個 Project 項目（spec「Project 切換」；design D6；direction-01-visual task 3.1）：
  // `<button data-action="select-project" data-project="...">`，焦點還原（render.js 檔尾
  // identityFromElement／findByFocusIdentity）自動涵蓋——不需要像 pane 列那樣額外處理
  // tabIndex／role／keydown（design D6：「左欄項目是 button，帶 data-action／data-project，
  // 焦點還原自動涵蓋」）。選定標示＝design D4「冰青的形狀分工」：--surface 底＋左緣 2px 冰青條
  // （style.css .project-item.selected），跟 running 節點的四邊框加柔光在形狀上分開，也跟
  // pane 列的 .selected 用同一套視覺語彙。
  function renderProjectItem(project, isSelected) {
    var item = el("button", "project-item" + (isSelected ? " selected" : ""));
    item.type = "button";
    item.setAttribute("data-action", "select-project");
    item.setAttribute("data-project", project.id);
    if (isSelected) {
      item.setAttribute("aria-current", "true");
    }

    var header = el("div", "project-item-header");
    var nameEl = el("span", "project-item-name", project.name);
    // 長字串換行顯示（design D3；design 審核檢查清單「200 字的 Project 名稱會截斷或換行，
    // 不撐破 220px 欄寬」）：完整內容另外放 title。
    nameEl.title = project.name;
    header.appendChild(nameEl);
    if (project.warnings.length > 0) {
      header.appendChild(renderWarningCount(project.warnings.length));
    }
    item.appendChild(header);

    var counts = computeTaskStatusCounts(project);
    if (counts.length > 0) {
      var countsRow = el("div", "project-item-counts");
      for (var i = 0; i < counts.length; i += 1) {
        countsRow.appendChild(renderStatusCountChip(counts[i]));
      }
      item.appendChild(countsRow);
    }

    return item;
  }

  // Repo Project 的管理選單（spec cockpit-dashboard「Project 切換」：`kind` 為 `repo` 的項目有「⋯」選單，手寫的
  // 沒有；repo-projects design D9、task 5.2）。只有 `kind === "repo"` 才包一層 `.project-entry` 放「⋯」，其餘（`config`
  // 與任何未知的新值）維持原本那顆 `.project-item` 按鈕，不加選單——新的 kind 不會被誤歸成可管理。
  // 「⋯」是項目按鈕的兄弟節點（按鈕不能巢狀）：`data-action="project-menu"`，`aria-expanded`／`aria-controls` 表達
  // 選單開關；選單開著時（actions.js 的 `ui.projectMenu`，模組狀態、不在 DOM，整頁重畫不會丟）在項目下方畫三顆按鈕
  // `project-rename`／`project-edit-stages`／`project-remove`。按下的處理、Esc、點別處關閉、對話框都在 actions.js。
  // 焦點還原依 data-* 身分自動涵蓋（data-action＋data-project）。
  function projectMenuId(projectId) {
    return "project-menu-" + encodeURIComponent(projectId);
  }

  function renderRepoProjectEntry(project, isSelected, menuOpen) {
    var entry = el("div", "project-entry");
    entry.appendChild(renderProjectItem(project, isSelected));
    var toggle = actionButton("⋯", { action: "project-menu", project: project.id });
    toggle.classList.add("project-menu-button");
    toggle.setAttribute("aria-label", t("render.projectMenu.label", { name: project.name }));
    toggle.title = t("render.projectMenu.label", { name: project.name });
    toggle.setAttribute("aria-expanded", menuOpen ? "true" : "false");
    toggle.setAttribute("aria-controls", projectMenuId(project.id));
    entry.appendChild(toggle);
    if (menuOpen) {
      var menu = el("div", "project-menu");
      menu.id = projectMenuId(project.id);
      menu.setAttribute("data-project", project.id);
      menu.setAttribute("role", "group");
      menu.setAttribute("aria-label", t("render.projectMenu.label", { name: project.name }));
      menu.appendChild(actionButton(t("render.projectMenu.rename"), { action: "project-rename", project: project.id }));
      menu.appendChild(
        actionButton(t("render.projectMenu.editStages"), { action: "project-edit-stages", project: project.id })
      );
      menu.appendChild(actionButton(t("render.projectMenu.remove"), { action: "project-remove", project: project.id }));
      entry.appendChild(menu);
    }
    return entry;
  }

  // 左欄（design D2 data-region="projects"；direction-01-visual task 2.1／3.1）：依
  // `state.projects` 順序列出每個 Project（spec「Project 切換」），`selectedProjectId` 是
  // `renderState()` 算好的「實際生效的選取」（已經套用過「找不到就用第一個」的退回規則，見
  // `resolveSelectedProject()`）——這裡只負責標示哪一項該顯示 `.selected`，不重算退回規則，
  // 避免兩處各自判斷、彼此不一致。投影沒有任何 Project 時顯示空狀態（spec「沒有 Project」）。
  function renderProjectsRegion(state, selectedProjectId, hidden, addingRepos, projectMenu) {
    var region = el("nav", "region-projects");
    region.setAttribute("data-region", "projects");
    // file-review task 4.1（design D6）：左欄目前分頁不是「Project」時整塊 hidden；左欄目前分頁是
    // files.js 的狀態，由 paint() 讀出後傳進來（見 renderState() 的第三參數）。
    region.hidden = hidden === true;

    if (state.projects.length === 0) {
      region.appendChild(renderProjectsEmptyState());
    } else {
      var list = el("div", "project-list");
      for (var i = 0; i < state.projects.length; i += 1) {
        var project = state.projects[i];
        var isSelected = project.id === selectedProjectId;
        list.appendChild(
          project.kind === "repo"
            ? renderRepoProjectEntry(project, isSelected, projectMenu === project.id)
            : renderProjectItem(project, isSelected)
        );
      }
      region.appendChild(list);
    }
    // repo-projects task 5.1：偵測區接在 Project 清單（或其空狀態）之後，沒有 Project 時也照畫——Factory Floor
    // 的空狀態文字指向這裡。
    region.appendChild(renderDetectedRepos(state.detected_repos, addingRepos));
    return region;
  }

  // 左欄「偵測到的 repo」區（spec cockpit-dashboard「Project 切換」；repo-projects design D9；task 5.1）：依投影
  // `detected_repos` 的順序列出每個 repo 的 `name`、`pane_count` 與「加入」鈕。順序、名稱、數量一律照投影給的欄位，
  // 不在前端重算（後端規則改了才不會漂移）。名稱以文字節點呈現（el() 的 textContent），不以 HTML 插入。
  // 「加入」鈕帶 data-action="add-repo"／data-repo（repo key 原樣送回）：焦點還原依 data-* 身分自動涵蓋；按下的處理
  // 與預設 stages 在 actions.js。沒有偵測到的 repo（或舊投影沒有這個欄位）時不列任何項目，只留標題與一行說明。
  // addingRepos（actions.js 的 ui.addingRepos 快照，repo key → 狀態）列著的 repo，「加入」呈現停用：aria-disabled="true"
  // ＋aria-busy="true"，不用 disabled 屬性——disabled 會讓焦點掉到 <body>，鍵盤使用者就失去位置（fix round 1）。
  // 按下的略過在 actions.js（依同一份狀態，不看 DOM）。
  function renderDetectedRepos(detectedRepos, addingRepos) {
    var adding = addingRepos || {};
    var repos = Array.isArray(detectedRepos) ? detectedRepos : [];
    var section = el("section", "detected-repos");
    var title = el("h2", "detected-repos-title", t("render.detected.title"));
    // 程式焦點的落點（repo-projects task 5.2 fix round 1）：焦點所在的 Project 消失、又沒有任何 Project 時，焦點放在這裡
    // （見 focusProjectFallback）。tabindex="-1"：可被程式聚焦、不進 Tab 順序。
    title.tabIndex = -1;
    // fix round 2：焦點還原認得 data-focus-id（identityFromElement／findByFocusIdentity），整頁重畫後焦點回到新的標題。
    // 刻意不用 data-action：actions.js 的點擊委派以 [data-action] 判斷操作，按標題不能變成一次「畫面操作」。
    title.setAttribute("data-focus-id", "detected-repos-title");
    section.appendChild(title);
    if (repos.length === 0) {
      section.appendChild(el("p", "detected-repos-empty", t("render.detected.empty")));
      return section;
    }
    var list = el("ul", "detected-repo-list");
    for (var i = 0; i < repos.length; i += 1) {
      var repo = repos[i];
      var item = el("li", "detected-repo");
      item.setAttribute("data-repo", repo.repo);
      var text = el("div", "detected-repo-text");
      var nameEl = el("span", "detected-repo-name", repo.name);
      // 長名稱單行省略（style.css），完整內容放 title（design D3）。
      nameEl.title = repo.name;
      text.appendChild(nameEl);
      text.appendChild(el("span", "detected-repo-count", tn("render.detected.paneCount", repo.pane_count)));
      item.appendChild(text);
      // 進行中（task 6.1 F1）：可見文字改「加入中…」、無障礙名稱改「加入中… <名稱>」（繁中以可見文字開頭，符合 label-in-name；
      // 英文「Adding <名稱>」），與單純的停用分得開；
      // style.css 給按鈕固定的 min-width，文字換了欄寬不跳動。
      var isAdding = Object.prototype.hasOwnProperty.call(adding, repo.repo);
      var add = actionButton(t(isAdding ? "render.detected.adding" : "render.detected.add"), { action: "add-repo", repo: repo.repo });
      // 每列都是「加入」，無障礙名稱另帶 repo 名稱才分得出是哪一個（可見文字在名稱開頭，符合 label-in-name）。
      add.setAttribute("aria-label", t(isAdding ? "render.detected.addingLabel" : "render.detected.addLabel", { name: repo.name }));
      if (isAdding) {
        add.setAttribute("aria-disabled", "true");
        add.setAttribute("aria-busy", "true");
      }
      item.appendChild(add);
      list.appendChild(item);
    }
    section.appendChild(list);
    return section;
  }

  // 「選取跨重畫保留」「未選定過時預設選定第一個」「選定的 Project 已不在最新投影中時改為
  // 選定第一個」三條規則（spec「Project 切換」；design D6）的唯一實作點：`renderState()`／
  // `renderProjectsRegion()` 都呼叫這裡，不各自重算，避免退回規則在兩處實作出現分歧。
  function resolveSelectedProject(state, selectedProjectId) {
    if (state.projects.length === 0) {
      return null;
    }
    if (selectedProjectId !== null && selectedProjectId !== undefined) {
      for (var i = 0; i < state.projects.length; i += 1) {
        if (state.projects[i].id === selectedProjectId) {
          return state.projects[i];
        }
      }
    }
    return state.projects[0];
  }

  // 底列通道狀態指示（design D4「連線配色也適用底列通道狀態」；direction-01-visual task
  // 2.3；task 2.3 fix round 1／Codex C1）：沿用「連線」列的三色與燈號形狀，文字前面加一個
  // 獨立的 span 標籤「cockpit 服務」，和頂列的 runtime 連線燈號（cockpit 到 HERDR 的連線）
  // 區分開，避免同一個「connected」字樣讓使用者分不出是哪一段連線斷了（design D4 原文）。
  // `#channel-status` 本身文字不變（只是 dot＋文字兩個子節點，textContent 仍然精確等於狀態
  // 字串），`window.onChannel` 仍然只找 `#channel-status` 更新，不觸發整頁重畫（見檔尾
  // window.onChannel）。
  //
  // Codex C1：這裡過去寫死 "connected"（理由是「既然這是隨整頁重畫畫出來的，代表 WebSocket
  // 當下是通的」）——這個理由只在「這次重畫是由收到新投影觸發」時成立，但 window.repaint()
  // 也會呼叫 paint()（例如使用者操作、UI-only 重畫，不是新投影抵達），若那時通道其實已經
  // 斷線，這裡仍然會把底列畫回綠色，蓋掉 window.onChannel 剛設定的紅色。改讀
  // latestChannelState（只由 window.onChannel 寫入），不再假設「有 repaint 就代表已連線」。
  function renderChannelIndicator() {
    var wrap = el("span", "statusbar-channel");
    wrap.appendChild(el("span", "statusbar-channel-label", t("index.channel.label")));
    // M4（設計審核，2.3 fix round 1）：說明這是「瀏覽器到 cockpit 服務」這一段連線，跟頂列的
    // runtime 燈號（cockpit 到 HERDR）區分開。
    wrap.title = t("index.channel.title", { state: latestChannelState });

    var badge = el("span", "channel-status " + connStateClass("channel-", latestChannelState));
    badge.id = "channel-status";
    badge.setAttribute("data-channel-state", "");

    // M3（使用者決定，2.3 fix round 1）：符號改用 .conn-symbol 共用規則（見
    // renderRuntimeLamp() 上方註解與 style.css），跟頂列燈號同一組三態形狀。
    var dot = el("span", "channel-status-dot conn-symbol");
    dot.setAttribute("aria-hidden", "true");
    badge.appendChild(dot);

    badge.appendChild(el("span", "channel-status-text", latestChannelState));

    wrap.appendChild(badge);
    return wrap;
  }

  // 底列（design D2 data-region="statusbar"；direction-01-visual task 2.3）：通道狀態
  // （design D4）＋ 程式版本（--fs-meta、--text-dim、等寬，design D10／D11：「version 用等寬
  // --text-dim 顯示，不搶注意力」）。2026-10-05 使用者指示：底列改顯示程式版本（例如 v0.1.2），
  // 投影的遞增 version 不再顯示、改放在同一節點的 data-state-version 屬性——驗收腳本以它判斷
  // 「第一份投影已畫出」與「發生了重畫」。
  var APP_VERSION = (function () {
    var meta = document.querySelector('meta[name="cockpit-version"]');
    var v = meta ? meta.getAttribute("content") || "" : "";
    // 沒經過後端替換（直接開原始 index.html）時是占位字，當成沒有版本。
    return /^__/.test(v) ? "" : v;
  })();

  function renderStatusbarRegion(state) {
    var region = el("div", "region-statusbar");
    region.setAttribute("data-region", "statusbar");

    region.appendChild(renderChannelIndicator());

    var version = el("span", "version", APP_VERSION ? "v" + APP_VERSION : "");
    version.id = "version";
    version.setAttribute("data-state-version", String(state.version));
    region.appendChild(version);

    return region;
  }

  // 純函數：state、選填的 ui → 一份新建的 DocumentFragment（design D2；direction-01-visual
  // task 2.1）。頂層輸出改成各區塊的平鋪清單（不再包一層 `.page`），每個區塊根節點帶
  // `data-region`——`#app` 在 index.html 設 `display: contents`，這些區塊因此直接是 `.shell`
  // 的 grid item，用 grid-area 就位（design D2「DOM 歸屬與版面位置分開」）。呼叫端
  // （paint()）用 `appEl.replaceChildren(renderState(...))`：`replaceChildren` 會展開
  // fragment，不接受陣列，呼叫端不用因此改參數型態。不讀寫 document 上既有的節點、不留任何
  // 全域狀態（`document.createDocumentFragment()`／`el()` 內的 `document.createElement()` 都只
  // 是節點工廠呼叫，不是讀寫既有 DOM）。
  // 第三參數 leftTab（file-review task 4.1／4.2；design D6）：左欄目前分頁（"projects" | "files" |
  // "changes"，缺省＝"projects"），決定 Project 清單是否 hidden——只有目前分頁**是**"projects"
  // 才顯示，其餘（含「檔案」與「變更」，以及未來任何新分頁）一律 hidden（fix round 2：4.2 加
  // 「變更」分頁時，這裡沿用了 file-review 4.1 時的二選一判斷 `leftTab === "files"`，讓「變更」
  // 分頁被誤判為「不是 files 所以顯示」，導致整頁重畫後 Project 清單在「變更」分頁時跑出來）。
  // 由 paint() 從 files.js 讀出後傳入，renderState() 本身仍不讀任何全域。
  function renderState(state, ui, leftTab) {
    var rebind = ui && ui.rebind ? ui.rebind : null;
    var error = ui && ui.error ? ui.error : null;
    var selected = ui && ui.selected ? ui.selected : null;
    var selectedProjectId = ui && ui.selectedProject ? ui.selectedProject : null;

    // spec「Project 切換」／design D6：三條規則（未選定過時預設第一個、選定的 Project 已不在
    // 最新投影中時改為第一個、選取跨重畫保留）都收斂在 resolveSelectedProject() 這一個呼叫，
    // 左欄（哪一項標 .selected）與 Factory Floor（畫哪個 Project）用同一份結果，不會對不齊。
    var selectedProject = resolveSelectedProject(state, selectedProjectId);

    var frag = document.createDocumentFragment();
    frag.appendChild(renderTopbar(state));

    // DOM 順序跟著 M7 裁決的視覺順序走（topbar→Project→banner→Floor→runtime→…），雖然
    // grid-template-areas 決定的是視覺位置、不是 DOM 順序（direction-01-visual task 2.1），
    // 但兩者一致比較好理解、鍵盤 Tab 順序也比較合理。
    frag.appendChild(
      renderProjectsRegion(
        state,
        selectedProject !== null ? selectedProject.id : null,
        leftTab !== "projects",
        ui && ui.addingRepos ? ui.addingRepos : null,
        ui && typeof ui.projectMenu === "string" ? ui.projectMenu : null
      )
    );

    var banner = renderBannerRegion(state, error, rebind);
    if (banner !== null) {
      frag.appendChild(banner);
    }

    // 中上區域只顯示目前選定的 Project 的 Factory Floor（spec「Factory Floor」「找不到就用
    // 第一個」；spec「兩個 Project」情境：「中上區域只有一張 Factory Floor……另一個的網格不在
    // 畫面上，改由左欄切換」；direction-01-visual task 3.1，取代 2.1～3.0 期間「把
    // state.projects 全部疊在一起畫」的暫時狀態，見 style.css fix round 3／N3 附近的註解）。
    // fix round 1（design 審核 I2）：外層區塊本身（data-region="floor"）改成不捲動的框，真正
    // 捲動的內層是沿用舊 class="projects" 的節點（factory-floor-check.js 逐字比對
    // `class="projects"` 在 `class="runtime-cards"` 之前；design D1「保留 DOM 身分」）——兩層
    // 分開之後，Factory Floor 的框線／未來 D8 切角掛在外層，不會跟著內層的捲動位置跑掉（3.2
    // 沿用同一層做 sticky 欄首／列首）。沒有任何 Project 時顯示空狀態（spec「沒有 Project」）。
    var floorRegion = el("div", "region-floor");
    floorRegion.setAttribute("data-region", "floor");
    if (selectedProject === null) {
      floorRegion.appendChild(renderFloorEmptyState());
    } else {
      // direction-01-visual task 3.2：標題列在外框、捲動容器之外（見 renderFloorTitlebar()）。
      floorRegion.appendChild(renderFloorTitlebar(selectedProject));
      var floorScroll = el("div", "projects");
      // sticky 列首蓋住捲動容器最左邊一段（首欄軌道最寬 FF_ROW_HEADER_MAX_PX）：
      // scrollIntoView／focus() 橫向捲動時要讓開它，目標才不會停在列首底下（上緣讓開欄首的
      // 部分見 style.css .projects 的 scroll-padding）。再多讓開 FOCUS_RING_ROOM_PX，焦點框才
      // 不會縮在列首底下（task 5.4 final review／Codex F2，四邊的理由見 style.css 同一處）。
      floorScroll.style.scrollPaddingLeft = FF_ROW_HEADER_MAX_PX + FOCUS_RING_ROOM_PX + "px";
      floorScroll.appendChild(renderProject(selectedProject));
      floorRegion.appendChild(floorScroll);
    }
    frag.appendChild(floorRegion);

    // fix round 1（design 審核 I2）：runtime 卡外層同樣拆成「不捲動的框
    // （data-region="runtimes"）＋捲動的內層（class="runtime-cards"，沿用舊名）」。
    var runtimesRegion = el("div", "region-runtimes");
    runtimesRegion.setAttribute("data-region", "runtimes");
    var cards = el("div", "runtime-cards");
    for (var i = 0; i < state.runtimes.length; i += 1) {
      cards.appendChild(renderRuntimeCard(state.runtimes[i], rebind !== null, selected));
    }
    runtimesRegion.appendChild(cards);
    frag.appendChild(runtimesRegion);

    frag.appendChild(renderRecentEvents(state.recent_events));

    frag.appendChild(renderStatusbarRegion(state));

    return frag;
  }

  // Live Output（spec live-output「選定一個 pane」；design D8；task 5.3）：交出目前投影裡還
  // 存在的所有 pane（跨 runtime、含 exited——exited 只是不可選，不是不存在），讓 output.js
  // 判斷被選定的 pane 是否已經從投影裡消失。同一個 pane id 可能出現在不同 runtime，所以一定
  // 要帶 runtime。每筆另帶 pane 的 cwd（null 表示沒有回報），給 files.js 判斷選定 pane 的根目錄是否該
  // 重查（file-review 最終修正波 F1；output.js 只看 runtime／paneId）。
  function collectKnownPanes(state) {
    var panes = [];
    for (var r = 0; r < state.runtimes.length; r += 1) {
      var runtime = state.runtimes[r];
      for (var w = 0; w < runtime.workspaces.length; w += 1) {
        var workspace = runtime.workspaces[w];
        for (var ti = 0; ti < workspace.tabs.length; ti += 1) {
          var tab = workspace.tabs[ti];
          for (var p = 0; p < tab.panes.length; p += 1) {
            var cwd = tab.panes[p].cwd;
            panes.push({ runtime: runtime.id, paneId: tab.panes[p].id, cwd: typeof cwd === "string" ? cwd : null });
          }
        }
      }
    }
    return panes;
  }

  // 最近一份投影：actions.js 改了 UI 狀態（進出改綁模式、錯誤訊息、選取）後，要用「最新投影
  // ＋新 UI 狀態」重畫，不必等下一次推送。
  var latestState = null;

  // 焦點還原（spec cockpit-dashboard「畫面整頁重畫」本文最後兩句；spec live-output「選定一個
  // pane」本文與情境「鍵盤焦點跨重畫保留」；live-output task focus-fix）：`replaceChildren`
  // 把整棵 `#app` 換成新節點，若鍵盤焦點原本在 `#app` 內某個可互動元素上，節點被換掉後焦點會
  // 掉回 `<body>`。重畫前後各做一次「身分」比對，找到就把焦點還原到新樹裡代表同一個對象的元素
  // 上；找不到（對象已消失、或新畫面裡已不可互動）就什麼都不做，讓焦點留在瀏覽器預設的
  // `<body>`。
  //
  // 盤點（task focus-fix）：#app 內目前所有可聚焦、可互動的元素都帶 `data-action`——pane 列
  // （`select-pane`，`data-runtime`／`data-pane`）、workstream 列首「看輸出」
  // （`select-bound-pane`，同兩個）、「綁定到這裡」（`bind-here`，同兩個）、task 節點的
  // 「推進」「退回」「Completed」「Failed」「清除標記」（`advance`／`retreat`／`complete`／`fail`／`clear`，
  // `data-project`／`data-task`）、workstream 列首「改綁」（`rebind`，
  // `data-project`／`data-workstream`）、「取消改綁」（`override-clear`，同兩個）、改綁提示的
  // 「取消」（`rebind-cancel`，只有 `data-action`）、錯誤訊息的「關閉」
  // （`error-dismiss`，只有 `data-action`）、頂列的通知鈴鐺（`notify-settings`，只有 `data-action`；
  // desktop-launch-notify task 3.3）、左欄偵測區的「加入」（`add-repo`，`data-repo`；repo-projects task 5.1）。
  // 沒有找到不帶 `data-action` 的可聚焦元素——若之後
  // 新增這種元素，下面的身分規則需要重新檢討。Live Output 面板（`#output`）不在 `#app`
  // 底下、完全不受這裡影響，本來就不需要焦點還原（design D8）。
  //
  // 身分＝`data-action` 加上該元素當下**全部**的 `data-*` 屬性（不逐一列名，直接比對整個
  // `dataset`）：同一個 `data-action` 底下的 data-* 組合是固定的，全量比對比手動列舉哪些欄位
  // 「足以指認」更不容易漏掉、也更不容易在之後新增 data-* 屬性時悄悄變得不準。找同一個對象時
  // 逐一比對 `dataset`（不組 CSS 屬性選擇器字串）——pane id 含冒號，組字串需要正確跳脫（見
  // brief），逐一比對天然沒有這個問題。
  // repo-projects task 5.2 fix round 2：不是操作、但要能跨重畫保留焦點的節點（偵測區標題）帶 data-focus-id，同樣當作身分。
  function identityFromElement(el) {
    if (!el || !el.dataset || (!el.dataset.action && !el.dataset.focusId)) {
      return null;
    }
    var identity = {};
    var key;
    for (key in el.dataset) {
      if (Object.prototype.hasOwnProperty.call(el.dataset, key)) {
        identity[key] = el.dataset[key];
      }
    }
    return identity;
  }

  function captureFocusIdentity(appEl) {
    var active = document.activeElement;
    if (!active || !appEl.contains(active)) {
      return null;
    }
    return identityFromElement(active);
  }

  // 待還原目標（fix round 1 Finding 1）：actions.js 的 pointerdown 委派在 #app 根節點同步
  // 呼叫 perform()，多數「畫面操作」（TASK_OPS、rebind、rebind-cancel、bind-here、
  // override-clear、error-dismiss，含 select-pane／select-bound-pane）都會在 perform() 內
  // 同步呼叫 window.repaint() → paint()。瀏覽器套用「把焦點移到被按下元素」這個預設動作的
  // 時機不保證早於這次同步 repaint（實測：Chromium 這次 paint() 開始時
  // document.activeElement 既不是按下前的舊焦點、也還不是剛被按下的目標——見
  // live-output-check.js U 段 RED 的實際輸出）——單靠 captureFocusIdentity() 看
  // document.activeElement，這次同步 repaint 永遠抓不到真正被按下的那個元素，重畫後鍵盤焦點
  // 就没有機會落在使用者剛剛按下的目標上。
  //
  // 修法：actions.js 在呼叫 perform(target) 之前，把 target 交給這裡（見檔尾
  // window.cockpitFocusHint.setPendingTarget）；paint() 開始時優先讀這個「待還原目標」的身分
  // （consumePendingFocusIdentity()），讀到就整個取代 captureFocusIdentity() 的結果（不再看
  // document.activeElement）——這正是 brief 要求的「優先還原到被按目標，找不到也不退回原本
  // 聚焦的元素」：一旦這次同步 repaint 是由 pointerdown 觸發，唯一的還原依據就是被按下的目標，
  // 找不到（該元素在新畫面已消失或不可互動）就什麼都不做，不會意外退回按下前的舊焦點。
  //
  // 「用完即清」：consumePendingFocusIdentity() 一律先把 pendingFocusTarget 讀出再清成
  // null，不管這次讀到的值有沒有真的被拿去用（例如身分裡沒有 data-action，理論上不會發生，
  // actionTarget() 已經保證委派到的一定是帶 data-action 的元素）——之後由 /ws 推送觸發的重畫
  // 一律看到 pendingFocusTarget 已經是 null，照一般規則走 document.activeElement，不會被這次
  // pointerdown 的殘留值誤導。actions.js 那一側也會在 perform() 呼叫前後各設一次（呼叫前設
  // target、呼叫後清成 null），是防禦性的第二層保險：萬一某次 pointerdown 委派到的
  // data-action 不被 perform() 任何分支接受（不會觸發 repaint()），這裡也不會把這次沒用到的
  // 目標留到下一次重畫。
  //
  // 光有這個待還原目標還不夠：實測發現這次同步 repaint 換掉 #app 之後，瀏覽器仍會為這次滑鼠
  // 按壓補送一組相容用的 mousedown／mouseup／click（Pointer Events 相容事件）——這組事件用
  // 「按下當下」的座標對新 DOM 重新 hit-test，命中的往往不是新按鈕本身而是外層容器，它的預設
  // 聚焦動作會把這裡剛還原好的焦點又搶走、吹到沒有任何元素聚焦。真正的修法在 actions.js：
  // pointerdown 委派呼叫 `event.preventDefault()` 關掉這組相容事件（Pointer Events 規格明文
  // 允許），這裡的還原邏輯本身不需要再額外處理時序（見 actions.js pointerdown listener 上方
  // 註解）。
  var pendingFocusTarget = null;

  function setPendingFocusTarget(el) {
    pendingFocusTarget = el || null;
  }

  // 焦點所在的 Project 已消失時的落點（repo-projects task 5.2 fix round 1）：實際選定的 Project 項目（render 標
  // aria-current="true" 的那一個）；沒有任何 Project 時落到偵測區標題。刻意不落在偵測區的「加入」：被移除的 repo 會回到
  // 偵測區，焦點若落在它的「加入」上，連按 Enter 會立刻重新加入。經 output.js 的共用 helper，依最後輸入方式決定外框。
  function focusProjectFallback(appEl, keepVisible) {
    var target =
      appEl.querySelector('[data-action="select-project"][aria-current="true"]') ||
      appEl.querySelector('[data-region="projects"] .detected-repos-title');
    if (target === null) {
      return;
    }
    if (window.cockpitFocus && typeof window.cockpitFocus.focus === "function") {
      window.cockpitFocus.focus(target, keepVisible === true);
    } else {
      target.focus({ preventScroll: true });
    }
  }

  window.cockpitFocusHint = {
    setPendingTarget: setPendingFocusTarget,
    // actions.js：對話框關閉時「⋯」已不在（Project 已消失）就呼叫這裡。
    focusProjectFallback: function () {
      var appEl = document.getElementById("app");
      if (appEl !== null) {
        focusProjectFallback(appEl, false);
      }
    },
  };

  function consumePendingFocusIdentity() {
    var target = pendingFocusTarget;
    pendingFocusTarget = null;
    return identityFromElement(target);
  }

  // identity 與 node.dataset 的鍵值集合完全相同才算同一個對象（雙向比對，不只是 identity 的
  // 鍵都在 dataset 裡）。
  function matchesFocusIdentity(node, identity) {
    var dataset = node.dataset;
    var key;
    for (key in identity) {
      if (Object.prototype.hasOwnProperty.call(identity, key) && dataset[key] !== identity[key]) {
        return false;
      }
    }
    for (key in dataset) {
      if (Object.prototype.hasOwnProperty.call(dataset, key) && !(key in identity)) {
        return false;
      }
    }
    return true;
  }

  // 理論上不該有多個元素同時符合同一個身分；真的發生時取文件順序中第一個（brief 邊界）。
  function findByFocusIdentity(appEl, identity) {
    if (identity === null) {
      return null;
    }
    var candidates = appEl.querySelectorAll("[data-action], [data-focus-id]");
    for (var i = 0; i < candidates.length; i += 1) {
      if (matchesFocusIdentity(candidates[i], identity)) {
        return candidates[i];
      }
    }
    return null;
  }

  // 找不到（對象已消失，或該元素在新畫面已不可互動，例如進入改綁模式後 pane 列不再有
  // `data-action`）就什麼都不做；`focus({ preventScroll: true })` 不得因為還原焦點而捲動頁面
  // （brief）。這裡只呼叫原生 `focus()`，不觸發任何 `perform()`／`repaint()`（`focus()`
  // 不會送出 `pointerdown`／`keydown`，專案裡也沒有任何 `focus` 事件 handler 會因此做事）。
  // 同步呼叫即可（fix round 1 Finding 1 真正的根因與修法在 actions.js 的 `preventDefault()`，
  // 見上方「待還原目標」註解最後一段與 actions.js pointerdown listener 上方註解）。
  function restoreFocus(appEl, identity, keepVisible) {
    var target = findByFocusIdentity(appEl, identity);
    if (target !== null && typeof target.focus === "function") {
      // 經 output.js 的共用 helper（ui-fixes task 4.2；design D1）：最後輸入為滑鼠時不呈現焦點外框。
      // output.js 沒載入的精簡 harness（只載 render.js／actions.js 的驗收腳本）退回原生 focus()。
      if (window.cockpitFocus && typeof window.cockpitFocus.focus === "function") {
        window.cockpitFocus.focus(target, keepVisible === true);
      } else {
        target.focus({ preventScroll: true });
      }
    }
  }

  // 內層捲動位置跨重畫保留（direction-01-visual task 2.1 fix round 4）：2.1 把 Factory
  // Floor、runtime 清單、最近事件改成「區塊是不捲的框、內層才捲動」（design 審核 I2），這些內層
  // 捲動容器都在 #app 底下，每次 replaceChildren 都換成新節點、scrollTop／scrollLeft 歸零——
  // 改版前捲的是整頁（document 不會被換掉），所以沒有這個問題。實測（COCKPIT_PREVIEW_PUSH_MS=100）：
  // 把三個容器捲到 120／30 後 400ms 內全部被拉回 0；actions-check.js「頻繁重畫時按鈕仍有效」
  // 因此在 scrollIntoView 之後、按下之前按鈕被捲走而漏送 POST（HEAD 上 4 次失敗 3 次）。
  // 重畫前依選擇器記下位置、重畫後寫回（新內容比較短時瀏覽器會自己夾到合法範圍）。
  var SCROLL_KEEP_SELECTORS = [
    '[data-region="floor"] > .projects',
    '[data-region="runtimes"] > .runtime-cards',
    '[data-region="events"] > .recent-events-list',
    // direction-01-visual task 3.1：左欄現在有真的內容，Project 數量夠多時會需要自己捲動——
    // 但跟其餘三個區塊不同，左欄捲動的是「框」本身（[data-region="projects"]，style.css 依
    // 版面分別給它 max-height+overflow-y:auto 或 overflow-y:auto，見該檔「左欄捲動策略」與
    // 固定一屏那段的註解），不是內層的 .project-list（.project-list 只負責排版，沒有自己的
    // overflow）。[data-region="projects"] 本身也是 renderState() 平鋪清單的一員、每次
    // replaceChildren 都被換掉，一樣需要跨重畫保留捲動位置。
    '[data-region="projects"]',
  ];

  // 捲動位置只在「同一份內容」重畫時保留（task 5.4 final review／Codex F1）：Factory Floor 的
  // 捲動容器每次只畫一個 Project，選擇器一樣不代表內容一樣——切換 Project 後若照選擇器寫回，
  // 上一個 Project 的捲動位置會套到新 Project 上（使用者從網格中段開始、看不到首欄與前面的
  // stage）。所以每筆記錄另外記下容器當時畫的 Project（直接子節點 .project 的 data-project；
  // 其餘三個容器沒有這個子節點，恆為 null），重畫後只有同一個 Project 才寫回；換了 Project
  // 就不寫回，新 Project 從 0 開始（切回原本的 Project 也是 0，不另外記每個 Project 的位置）。
  function scrollOwner(node) {
    var project = node.querySelector(":scope > .project");
    return project !== null ? project.getAttribute("data-project") : null;
  }

  function captureScroll(appEl) {
    var saved = [];
    for (var i = 0; i < SCROLL_KEEP_SELECTORS.length; i += 1) {
      var node = appEl.querySelector(SCROLL_KEEP_SELECTORS[i]);
      if (node !== null && (node.scrollTop !== 0 || node.scrollLeft !== 0)) {
        saved.push({
          selector: SCROLL_KEEP_SELECTORS[i],
          owner: scrollOwner(node),
          top: node.scrollTop,
          left: node.scrollLeft,
        });
      }
    }
    return saved;
  }

  function restoreScroll(appEl, saved) {
    for (var i = 0; i < saved.length; i += 1) {
      var node = appEl.querySelector(saved[i].selector);
      if (node !== null && scrollOwner(node) === saved[i].owner) {
        node.scrollTop = saved[i].top;
        node.scrollLeft = saved[i].left;
      }
    }
  }

  // 投影的偵測區是否還列著這個 repo key（焦點還原判斷「加入」鈕是否還會在新畫面上）。
  function latestStateHasDetectedRepo(state, repo) {
    var repos = Array.isArray(state.detected_repos) ? state.detected_repos : [];
    for (var i = 0; i < repos.length; i += 1) {
      if (repos[i].repo === repo) {
        return true;
      }
    }
    return false;
  }

  function latestStateHasProject(state, id) {
    var projects = Array.isArray(state.projects) ? state.projects : [];
    for (var i = 0; i < projects.length; i += 1) {
      if (projects[i] && projects[i].id === id) {
        return true;
      }
    }
    return false;
  }

  function paint() {
    if (latestState === null) {
      return;
    }
    // repo-projects task 5.1（spec「Project 切換」：加入成功後，含新 Project id 的投影到達時自動選定它，只選一次）：
    // 待選定的 id 存在 actions.js 的模組狀態裡，這裡每次重畫前交出最新投影讓它判斷；選定了就回傳該 id。
    var autoSelected =
      window.cockpitActions && typeof window.cockpitActions.applyPendingProjectSelection === "function"
        ? window.cockpitActions.applyPendingProjectSelection(latestState)
        : null;
    // fix round 1：「加入」已成功的標記在偵測區不再列著該 repo 時刪掉（見 actions.js pruneAddingRepos）。
    if (window.cockpitActions && typeof window.cockpitActions.pruneAddingRepos === "function") {
      window.cockpitActions.pruneAddingRepos(latestState);
    }
    // repo-projects task 5.2：開著選單的 Project 已不在投影中（或不再是 Repo Project）時收起選單。
    if (window.cockpitActions && typeof window.cockpitActions.pruneProjectMenu === "function") {
      window.cockpitActions.pruneProjectMenu(latestState);
    }
    var ui =
      window.cockpitActions && typeof window.cockpitActions.uiSnapshot === "function"
        ? window.cockpitActions.uiSnapshot()
        : undefined;
    // spec「Project 切換」「選定的 Project 已不在最新投影中時改為選定第一個」（design D6；
    // direction-01-visual task 3.1 fix round 1／Codex finding）：這是正式的狀態改變，不只是
    // 這次重畫的顯示 fallback——resolveSelectedProject() 原本只被 renderState() 拿來決定
    // 「這次要畫哪個 Project」，沒有回寫 actions.js 那份持久狀態，若選定的 Project 只是暫時
    // 從投影裡消失（例如短暫的投影抖動）、之後又出現，殘留的舊 ID 會讓畫面在使用者沒有任何
    // 操作的情況下自己跳回去。只在「曾經明確選過某個 Project（ui.selectedProject 不是
    // null）」時才回寫；未選定過的情形維持「動態預設第一個」，不會把某個當下剛好排第一的
    // Project 鎖成往後的預設（呼叫 setSelectedProject() 見該函式上方註解：只改狀態、不觸發
    // 另一次 repaint，這次重畫本來就會用 resolveSelectedProject() 算出同樣的 fallback 結果，
    // 畫面已經正確）。
    //
    // fix round 2／Codex finding：round 1 只在 resolveSelectedProject() 回傳非 null（投影裡
    // 還有其他 Project 可以 fallback）時才回寫，投影 projects 剛好是空陣列時
    // resolveSelectedProject() 回傳 null，被 `!== null` 擋掉、完全不回寫——這裡殘留的舊 ID
    // （例如 p）沒有被清掉；下一份投影若又恢復成 [cockpit, p]，殘留的舊 ID 仍然「找得到」
    // （p 這次真的在投影裡），會被判定成「還是選定的」而直接跳回 p，不是「未選定過時預設第一
    // 個」的 cockpit。修法：把「要不要回寫」與「回寫成什麼」分開算——`resolveSelectedProject()`
    // 回傳 null 就正規化成 null（沒有 Project 可選，選取本身也該清空，等下一份非空投影再靠
    // 「未選定過時預設第一個」自然選出 cockpit，不是提前鎖定任何 ID）、回傳某個 Project 就
    // 正規化成它的 id；只要算出來的值跟目前記錄的不一樣就回寫。
    if (ui && ui.selectedProject !== null && ui.selectedProject !== undefined) {
      var resolvedForPersist = resolveSelectedProject(latestState, ui.selectedProject);
      var normalizedSelectedProject = resolvedForPersist !== null ? resolvedForPersist.id : null;
      if (normalizedSelectedProject !== ui.selectedProject) {
        if (window.cockpitActions && typeof window.cockpitActions.setSelectedProject === "function") {
          window.cockpitActions.setSelectedProject(normalizedSelectedProject);
        }
      }
    }
    var appEl = document.getElementById("app");
    // fix round 1 Finding 1：pointerdown 觸發的同步重畫優先用「待還原目標」（見上方
    // consumePendingFocusIdentity 註解），沒有才照舊看 document.activeElement。
    var pendingIdentity = consumePendingFocusIdentity();
    var focusIdentity = pendingIdentity || captureFocusIdentity(appEl);
    // ui-fixes 修正波 1 F-M1：不是 pointerdown 觸發的重畫（沒有待還原目標）時，若舊的焦點元素本來就
    // 匹配 :focus-visible（鍵盤使用者 Tab 到的），還原時要保留外框——即使最後輸入是 pointer
    // （按住捲軸之類不移動焦點的滑鼠操作也會觸發 pointerdown）。pointerdown 觸發的重畫是使用者剛按下
    // 了某個元素，外框不該出現，維持 false。
    var keepFocusVisible =
      pendingIdentity === null &&
      !!window.cockpitFocus &&
      typeof window.cockpitFocus.focusVisible === "function" &&
      window.cockpitFocus.focusVisible(document.activeElement);
    // repo-projects task 5.1：鍵盤焦點原本在「加入」鈕上、這次重畫自動選定了新 Project 時，那顆鈕會隨 repo 離開
    // 偵測區而消失，焦點改還原到新選定的 Project 項目上（不讓焦點掉回 <body>）。鈕還在（例如同時有別的 repo）就照舊。
    if (
      autoSelected !== null &&
      focusIdentity !== null &&
      focusIdentity.action === "add-repo" &&
      !latestStateHasDetectedRepo(latestState, focusIdentity.repo)
    ) {
      focusIdentity = { action: "select-project", project: autoSelected };
    }
    // repo-projects task 5.2 fix round 1：焦點在某個 Project 自己的控制項上（項目、「⋯」、選單項目），而這份投影已沒有
    // 這個 Project（例如移除成功、或在別處被移除），那些控制項都會消失；改落到 focusProjectFallback() 的落點，不讓焦點
    // 掉回 <body>。
    var focusOnVanishedProject =
      focusIdentity !== null &&
      typeof focusIdentity.project === "string" &&
      /^(select-project|project-)/.test(focusIdentity.action || "") &&
      !latestStateHasProject(latestState, focusIdentity.project);
    var savedScroll = captureScroll(appEl);
    // file-review task 4.1（design D6）：左欄目前分頁是 files.js 的模組狀態，每次重畫都重新讀。
    var leftTab =
      window.cockpitFiles && typeof window.cockpitFiles.leftTab === "function"
        ? window.cockpitFiles.leftTab()
        : "projects";
    // ui-fixes task 4.3（design D2）：通道狀態掛在 #app 根節點，style.css 在
    // `#app:not([data-channel-state="connected"])` 範圍內把來自投影的即時狀態色轉為最後已知的 --text-dim。
    // replaceChildren 只換子節點、根節點屬性本會跨重畫保留；這裡仍依模組變數 latestChannelState 寫回，
    // 避免任何路徑重建或清掉根節點屬性時遺失。先於換子節點寫入，新畫面第一個影格就是正確的顏色。
    appEl.setAttribute("data-channel-state", latestChannelState);
    appEl.replaceChildren(renderState(latestState, ui, leftTab));
    restoreScroll(appEl, savedScroll);
    // #output 不在 #app 底下、不被上面這行換掉（design D8）；每次重畫後仍要交出最新的 pane
    // 集合，讓 output.js 判斷被選的 pane 是否已經消失。
    var knownPanes = collectKnownPanes(latestState);
    if (window.liveOutput && typeof window.liveOutput.setKnownPanes === "function") {
      window.liveOutput.setKnownPanes(knownPanes);
    }
    // 檔案樹（#files）同樣不在 #app 底下：交出同一份 pane 集合（含 cwd），選定 pane 的 cwd 改變時由
    // files.js 重查根目錄（spec file-review「左欄檔案樹」：根目錄改變時讀取；最終修正波 F1）。
    if (window.cockpitFiles && typeof window.cockpitFiles.setKnownPanes === "function") {
      window.cockpitFiles.setKnownPanes(knownPanes);
    }
    // 左欄「變更」面板（#changes-panel；git-review task 4.2）同樣不在 #app 底下、同一份 pane 集合：
    // 選定 pane 的 cwd 改變、且「變更」分頁目前可見時，git.js 重查根目錄。
    if (window.cockpitGit && typeof window.cockpitGit.setKnownPanes === "function") {
      window.cockpitGit.setKnownPanes(knownPanes);
    }
    restoringFocus = true;
    try {
      if (focusOnVanishedProject) {
        focusProjectFallback(appEl, keepFocusVisible);
      } else {
        restoreFocus(appEl, focusIdentity, keepFocusVisible);
      }
    } finally {
      restoringFocus = false;
    }
    // project-select-pane task 1.1（spec cockpit-dashboard「Project 切換」）：因選定 Project 而選定 pane 時，把右欄
    // 該 pane 列捲進右欄捲動容器的可視範圍（只捲 .runtime-cards、不捲整頁，fix round 1 I1）。在新畫面建好、捲動位置與
    // 焦點都還原之後才做（restoreScroll 不會把它蓋回去）；只捲動、不移動焦點。沒有待捲動時什麼都不做，一般重畫不受影響。
    if (window.cockpitActions && typeof window.cockpitActions.flushPaneRowScroll === "function") {
      window.cockpitActions.flushPaneRowScroll();
    }
  }

  // 鍵盤焦點進到 Factory Floor 時把整顆按鈕連同焦點框捲進來（task 5.4 final review／Codex
  // F2）：style.css 的 .projects scroll-padding 四邊替焦點框留了位置，但 Chrome 用 Tab 移動焦點
  // 時，橫向只要元素還露出 32px 以上就當作「看得見」、完全不橫向捲動（實測：右緣按鈕本體被裁掉
  // 20px、焦點框整側不見，scrollLeft 不動）。`scrollIntoView({ inline: "nearest" })` 沒有這個
  // 門檻、會遵守 scroll-padding，所以在焦點事件裡補捲一次。只處理鍵盤焦點（:focus-visible）；
  // paint() 自己還原焦點時（restoringFocus）不捲——否則使用者捲開之後，每次重畫都會把畫面拉回
  // 焦點所在的按鈕，違反「重畫不重置區塊內部捲動位置」。
  var restoringFocus = false;
  var appRoot = document.getElementById("app");
  if (appRoot !== null) {
    appRoot.addEventListener("focusin", function (event) {
      var target = event.target;
      if (restoringFocus || !(target instanceof Element)) {
        return;
      }
      if (target.closest('[data-region="floor"] > .projects') === null || !target.matches(":focus-visible")) {
        return;
      }
      target.scrollIntoView({ block: "nearest", inline: "nearest" });
    });
  }

  // 桌面通知（design D7；desktop-launch-notify task 3.3）：重畫之後才把同一份狀態交給
  // notify.js 比對——通知判斷不影響畫面，且畫面先反映新狀態。`window.cockpitNotify` 不存在時
  // （只載入 render.js 的精簡 harness）略過。比對基準由 notify.js 自己保留，通道重連不重設。
  window.onState = function (state) {
    latestState = state;
    paint();
    if (window.cockpitNotify && typeof window.cockpitNotify.observe === "function") {
      window.cockpitNotify.observe(state);
    }
  };

  // 給 actions.js：UI 狀態改變後以最新投影重畫（沒有收過投影時什麼都不做）。
  window.repaint = paint;

  // 給 actions.js 的 selectPane（design D7；desktop-launch-notify task 3.3）：點 pane 通知時要確認
  // 該 pane 還在最新一份投影中；只讀、不複製（呼叫端不得修改）。還沒收過投影時回 null。
  window.cockpitLatestState = function () {
    return latestState;
  };

  // 通道狀態更新（spec cockpit-dashboard「畫面整頁重畫」；design D4「連線配色也適用底列通道
  // 狀態」；direction-01-visual task 2.3；task 2.3 fix round 1／Codex C1／使用者決定 I2；
  // fix round 2／N5；ui-fixes task 4.3 另寫 #app 的 data-channel-state，見函式內）：不呼叫
  // window.repaint()／paint()，不觸發整頁重畫。四件事都在這裡做：
  //   1. 更新 latestChannelState（Codex C1）——下一次整頁重畫（不管是新投影還是 UI-only
  //      repaint()）都會用這個值重新產生 renderChannelIndicator()／renderTopbar() 的輸出，
  //      不會再被寫死的 "connected" 蓋掉。
  //   2. 更新 [data-region="topbar"] 的 data-channel-state 屬性（I2）——CSS 依這個屬性把
  //      頂列燈號調暗＋顯示「最後已知」（見 style.css），不需要在 JS 這裡逐一碰每顆燈號的
  //      顏色或顯示狀態。
  //   3. 逐顆更新頂列燈號的 title（N5）——顏色／文字靠 CSS 屬性選擇器不重畫就能切換，但
  //      title 不是 CSS 能控制的東西，要逐一改寫。每顆燈號自己的連線狀態（connState）不會
  //      因為通道變動而改變，從 `[data-conn-state]` 子節點的 textContent 讀回來（跟
  //      renderRuntimeLamp() 產生這個節點時的用途一致：既給腳本精確讀值，也給這裡讀回原值），
  //      不需要另外存一份 data 屬性。
  //   4. 更新 #channel-status 內的文字節點與 class（沿用 fix round 1 之前的做法）。
  // #channel-status 底下的 .channel-status-dot（燈號符號，靠 currentColor 跟著
  // #channel-status 自己的顏色走）：直接整個 textContent = status 會把這個 dot 節點一併
  // 沖掉，所以改成只找 .channel-status-text 這個子節點寫文字。找不到（例如舊快取頁面／未來
  // 結構被改動）才退回整個 textContent 覆寫，維持防禦性。
  window.onChannel = function (status) {
    latestChannelState = status;

    // ui-fixes task 4.3（design D2）：#app 根節點的 data-channel-state——右欄、中欄、左欄計數的
    // 「最後已知」轉暗全靠它（style.css），不重畫就立即生效；paint() 結束前也會依 latestChannelState
    // 寫回同一個屬性。
    var appEl = document.getElementById("app");
    if (appEl) {
      appEl.setAttribute("data-channel-state", status);
    }

    var topbar = document.querySelector('[data-region="topbar"]');
    if (topbar) {
      topbar.setAttribute("data-channel-state", status);
      var lamps = topbar.querySelectorAll(".runtime-lamp[data-runtime]");
      for (var i = 0; i < lamps.length; i += 1) {
        var lamp = lamps[i];
        var runtimeId = lamp.getAttribute("data-runtime");
        var stateNode = lamp.querySelector("[data-conn-state]");
        var connState = stateNode ? stateNode.textContent.trim() : "";
        lamp.title = lampTitle(runtimeId, connState, status);
      }
    }

    var badge = document.getElementById("channel-status");
    if (!badge) {
      return;
    }
    var textEl = badge.querySelector(".channel-status-text");
    if (textEl) {
      textEl.textContent = status;
    } else {
      badge.textContent = status;
    }
    badge.className = "channel-status " + connStateClass("channel-", status);

    // M4：title 也要跟著更新——這是 .statusbar-channel（badge 的父層 wrap），不是 badge 本身
    // （wrap.title 在 renderChannelIndicator() 設，見上方）。
    var wrap = badge.closest(".statusbar-channel");
    if (wrap) {
      wrap.title = t("index.channel.title", { state: status });
    }
  };
})();
