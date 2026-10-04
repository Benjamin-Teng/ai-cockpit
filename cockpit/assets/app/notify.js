// notify.js：桌面通知與通知設定面板（spec desktop-notifications「通知事件」「通知呈現」「通知設定」；
// change desktop-launch-notify design D7、D8；desktop-launch-notify task 3.3）。
//
// 對外只有 `window.cockpitNotify = { observe, togglePanel, isOpen, diff, toNotifications }`：
//   - `diff(prev, next) → events`、`toNotifications(events, settings) → [{ title, body, tag, target }]`
//     是純函式：不碰 DOM、不讀寫模組狀態，驗收腳本可以直接呼叫（notify-check.js P 段）。
//   - `observe(state)`：render.js 的 `window.onState` 在 `paint()` 之後呼叫。第一份狀態只當比對基準；
//     之後每份都跟「上一份收到的狀態」比對。基準只在這裡保留、通道重連不重設（spec「重連後補報」）。
//   - `togglePanel()`：actions.js 的 `perform()` 收到鈴鐺（`data-action="notify-settings"`）時呼叫。
//   - `isOpen()`：設定面板是否開著；render.js 重建鈴鐺時據此設 `aria-expanded`（修正波，3.5 採納項）。
//
// 副作用集中在這裡：權限判斷、`new Notification`、點選通知、本機儲存、設定面板。
//   - 權限不是 "granted"、或瀏覽器沒有 Notification 時不發、不報錯，也不主動請求權限（只有面板的
//     「允許通知」按鈕會請求）。
//   - 前景判斷（design D7）：`document.visibilityState === "visible" && document.hasFocus()` 時不發。
//   - 建立通知帶識別標籤與 `renotify: true`：同一個 pane／task 的同類事件以新通知取代舊通知，且取代時
//     仍重新提醒（spec「通知呈現」）。
//   - 點選通知：`window.focus()`；pane 事件另呼叫 `window.cockpitActions.selectPane(runtime, paneId)`
//     （改綁模式中或 pane 已不在最新投影時它不做事）；最後 `close()`。
//
// 設定面板（design D8）：`body` 底下、`#app` 之外的獨立節點（`#notify-panel`），載入時建好、以
// `hidden` 開關。整頁重畫只換 `#app` 的子節點，不影響它（memory「整頁重畫會丟掉只存在 DOM 上的
// 狀態」）。鈴鐺本身由 render.js 的 renderTopbar 每次重畫重建，所以這裡每次都重新查目前的鈴鐺。
// 開啟時焦點移到第一個開關；Esc、再按一次鈴鐺、點面板與鈴鐺以外的位置都會關閉，關閉時若焦點原本在
// 面板內就回到鈴鐺。程式焦點一律經 output.js 的 `window.cockpitFocus.focus`：最後輸入是滑鼠時不畫
// 焦點外框（memory「滑鼠操作沒聚焦任何元素時，之後的程式 focus() 會被 Chrome 判成 :focus-visible」）。
//
// 設定存在 localStorage 鍵 `cockpit.notify.v1`（四個布林）；讀取逐鍵驗證型別，缺漏或損毀用預設值；
// 別的同源視窗改了設定時（`storage` 事件）重讀並同步開著的面板；
// 本機儲存不可用（存取 `window.localStorage` 本身就丟例外）時只存在記憶體，照常生效、不報錯。
//
// 載入順序：index.html 在 render.js 之前載入本檔（render.js 會呼叫 observe）。script 在 body 尾端，
// 執行時 `document.body` 已存在。

(function () {
  "use strict";

  var STORAGE_KEY = "cockpit.notify.v1";
  var KINDS = ["blocked", "done", "failed", "completed"];
  var DEFAULTS = { blocked: true, done: false, failed: true, completed: false };
  // 同一份狀態最多逐一發出的則數；超過就合併成一則（spec「通知呈現」）。
  var MAX_SINGLE = 3;
  var BELL_SELECTOR = '#app [data-action="notify-settings"]';

  var t = window.cockpitI18n.t;
  var tn = window.cockpitI18n.tn;

  // 面板上的標籤與說明（spec「通知設定」「介面文字涵蓋範圍」）：四個事件名稱是產品詞彙，兩種語言都照原文；
  // 說明走字典（`notify.kind.<kind>.desc`）。描述 HERDR done 的字典鍵照 design D5 命名（`notify.kind.done.desc`、
  // `notify.title.done`），由 cockpit/tests/http.rs 守門：值不得暗示 task 已結束。
  var KIND_LABELS = {
    blocked: { name: "agent blocked", desc: t("notify.kind.blocked.desc") },
    done: { name: "agent done", desc: t("notify.kind.done.desc") },
    failed: { name: "task failed", desc: t("notify.kind.failed.desc") },
    completed: { name: "task completed", desc: t("notify.kind.completed.desc") },
  };

  // 通知標題（spec「通知呈現」）：task failed／task completed 是事件名稱，兩種語言同文。
  var TITLES = {
    blocked: t("notify.title.blocked"),
    done: t("notify.title.done"),
    failed: "task failed",
    completed: "task completed",
  };

  // ---------------------------------------------------------------------------
  // 純函式
  // ---------------------------------------------------------------------------

  function arr(value) {
    return Array.isArray(value) ? value : [];
  }

  // 每個 pane（含所在 runtime id），依投影順序。
  function eachPane(state, fn) {
    var runtimes = arr(state && state.runtimes);
    for (var r = 0; r < runtimes.length; r += 1) {
      var runtime = runtimes[r];
      if (!runtime) {
        continue;
      }
      var workspaces = arr(runtime.workspaces);
      for (var w = 0; w < workspaces.length; w += 1) {
        var tabs = arr(workspaces[w] && workspaces[w].tabs);
        for (var ti = 0; ti < tabs.length; ti += 1) {
          var panes = arr(tabs[ti] && tabs[ti].panes);
          for (var p = 0; p < panes.length; p += 1) {
            if (panes[p]) {
              fn(runtime.id, panes[p]);
            }
          }
        }
      }
    }
  }

  // pane 以 runtime id ＋ pane id 識別（spec「通知事件」）。
  function paneKey(runtimeId, paneId) {
    return JSON.stringify([runtimeId, paneId]);
  }

  function paneIndex(state) {
    var index = Object.create(null);
    eachPane(state, function (runtimeId, pane) {
      index[paneKey(runtimeId, pane.id)] = pane;
    });
    return index;
  }

  function taskIndex(state) {
    var index = Object.create(null);
    var projects = arr(state && state.projects);
    for (var i = 0; i < projects.length; i += 1) {
      var project = projects[i];
      if (!project) {
        continue;
      }
      var tasks = arr(project.tasks);
      for (var j = 0; j < tasks.length; j += 1) {
        if (tasks[j]) {
          index[JSON.stringify([project.id, tasks[j].id])] = tasks[j];
        }
      }
    }
    return index;
  }

  // 綁定到這個 pane 的 workstream 名稱（binding.state 為 bound 且 runtime 與 pane id 都相符；
  // 名稱去重、依投影順序）。
  function boundNames(state, runtimeId, paneId) {
    var names = [];
    var projects = arr(state && state.projects);
    for (var i = 0; i < projects.length; i += 1) {
      var workstreams = arr(projects[i] && projects[i].workstreams);
      for (var j = 0; j < workstreams.length; j += 1) {
        var ws = workstreams[j];
        var binding = ws && ws.binding;
        if (
          binding &&
          binding.state === "bound" &&
          binding.runtime === runtimeId &&
          binding.pane_id === paneId
        ) {
          var name = typeof ws.name === "string" && ws.name !== "" ? ws.name : String(ws.id);
          if (names.indexOf(name) === -1) {
            names.push(name);
          }
        }
      }
    }
    return names;
  }

  // 比對兩份狀態，回傳通知事件（不看設定；設定由 toNotifications 套用）。
  //   pane 事件：{ kind: "blocked" | "done", runtime, pane, names }
  //   task 事件：{ kind: "failed" | "completed", project, task, projectName, taskTitle }
  // 上一份沒有的 pane／task（新出現的）不產生事件；exited 為真的 pane 不產生事件；值沒變不產生事件。
  function diff(prev, next) {
    var events = [];
    if (!prev || !next) {
      return events;
    }
    var prevPanes = paneIndex(prev);
    eachPane(next, function (runtimeId, pane) {
      var status = pane.agent_status;
      if (status !== "blocked" && status !== "done") {
        return;
      }
      if (pane.exited === true) {
        return;
      }
      var before = prevPanes[paneKey(runtimeId, pane.id)];
      if (!before || before.agent_status === status) {
        return;
      }
      events.push({
        kind: status,
        runtime: runtimeId,
        pane: pane.id,
        names: boundNames(next, runtimeId, pane.id),
      });
    });

    var prevTasks = taskIndex(prev);
    var projects = arr(next.projects);
    for (var i = 0; i < projects.length; i += 1) {
      var project = projects[i];
      if (!project) {
        continue;
      }
      var tasks = arr(project.tasks);
      for (var j = 0; j < tasks.length; j += 1) {
        var task = tasks[j];
        if (!task || (task.status !== "failed" && task.status !== "completed")) {
          continue;
        }
        var before = prevTasks[JSON.stringify([project.id, task.id])];
        if (!before || before.status === task.status) {
          continue;
        }
        events.push({
          kind: task.status,
          project: project.id,
          task: task.id,
          projectName: typeof project.name === "string" && project.name !== "" ? project.name : String(project.id),
          taskTitle: typeof task.title === "string" && task.title !== "" ? task.title : String(task.id),
        });
      }
    }
    return events;
  }

  // 設定逐鍵驗證：每個類別必須是布林，否則用預設值。
  function normalizeSettings(raw) {
    var out = {};
    for (var i = 0; i < KINDS.length; i += 1) {
      var kind = KINDS[i];
      out[kind] =
        raw && typeof raw === "object" && typeof raw[kind] === "boolean" ? raw[kind] : DEFAULTS[kind];
    }
    return out;
  }

  function single(event) {
    if (event.kind === "blocked" || event.kind === "done") {
      var body =
        event.names && event.names.length > 0
          ? t("notify.body.paneNames", {
              runtime: event.runtime,
              pane: event.pane,
              names: event.names.join(t("notify.body.namesSeparator")),
            })
          : t("notify.body.pane", { runtime: event.runtime, pane: event.pane });
      return {
        title: TITLES[event.kind],
        body: body,
        tag: "cockpit:" + event.kind + ":" + event.runtime + "/" + event.pane,
        target: { runtime: event.runtime, paneId: event.pane },
      };
    }
    return {
      title: TITLES[event.kind],
      body: t("notify.body.task", { project: event.projectName, task: event.taskTitle }),
      tag: "cockpit:" + event.kind + ":" + event.project + "/" + event.task,
      target: null,
    };
  }

  // 套用設定（關閉的類別不產生通知、不計入件數），1 至 3 件逐一，超過 3 件合併成一則：
  // 標題「Cockpit：N 件事需要注意」（英文 `Cockpit: N items need attention`），內文列前 3 件、最後一行「…」，標籤 cockpit:summary。
  function toNotifications(events, settings) {
    var enabled = normalizeSettings(settings);
    var list = [];
    var items = arr(events);
    for (var i = 0; i < items.length; i += 1) {
      if (items[i] && enabled[items[i].kind] === true && TITLES[items[i].kind]) {
        list.push(single(items[i]));
      }
    }
    if (list.length <= MAX_SINGLE) {
      return list;
    }
    var lines = [];
    for (var j = 0; j < MAX_SINGLE; j += 1) {
      lines.push(t("notify.summary.line", { title: list[j].title, body: list[j].body }));
    }
    lines.push("…");
    return [
      {
        title: tn("notify.summary.title", list.length),
        body: lines.join("\n"),
        tag: "cockpit:summary",
        target: null,
      },
    ];
  }

  // ---------------------------------------------------------------------------
  // 設定（本機儲存）
  // ---------------------------------------------------------------------------

  function loadSettings() {
    var raw = null;
    try {
      raw = window.localStorage.getItem(STORAGE_KEY);
    } catch (e) {
      raw = null;
    }
    if (typeof raw !== "string") {
      return normalizeSettings(null);
    }
    try {
      return normalizeSettings(JSON.parse(raw));
    } catch (e) {
      return normalizeSettings(null);
    }
  }

  function saveSettings() {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch (e) {
      // 本機儲存不可用：只存在記憶體，這次載入期間照常生效。
    }
  }

  var settings = loadSettings();

  // ---------------------------------------------------------------------------
  // 發出通知
  // ---------------------------------------------------------------------------

  function notificationApi() {
    try {
      return typeof window.Notification === "function" ? window.Notification : null;
    } catch (e) {
      return null;
    }
  }

  function permission() {
    var api = notificationApi();
    if (api === null) {
      return "unsupported";
    }
    try {
      return String(api.permission);
    } catch (e) {
      return "default";
    }
  }

  function inForeground() {
    try {
      return document.visibilityState === "visible" && document.hasFocus();
    } catch (e) {
      return false;
    }
  }

  function onNotificationClick(note, notification) {
    try {
      window.focus();
    } catch (e) {
      // 帶到前景失敗不影響其餘動作。
    }
    if (
      note.target &&
      window.cockpitActions &&
      typeof window.cockpitActions.selectPane === "function"
    ) {
      window.cockpitActions.selectPane(note.target.runtime, note.target.paneId);
    }
    try {
      notification.close();
    } catch (e) {
      // 已關閉。
    }
  }

  function show(note) {
    var api = notificationApi();
    if (api === null) {
      return;
    }
    var notification;
    try {
      // icon：應用程式圖示（change app-icon design D6），與 index.html 的 favicon 同一張。
      notification = new api(note.title, { body: note.body, tag: note.tag, renotify: true, icon: "/icons/icon-192.png" });
    } catch (e) {
      // 某些環境不允許在頁面直接建構 Notification：不報錯（spec「通知呈現」）。
      return;
    }
    notification.onclick = function () {
      onNotificationClick(note, notification);
    };
  }

  var lastState = null;

  function observe(state) {
    var prev = lastState;
    lastState = state;
    // 整頁重畫（在這之前已完成）可能改變頂列高度（窄版燈號換行、斷線多「最後已知」）：面板開著時
    // 跟著鈴鐺重新定位（修正波 3.6 M4）。
    if (isOpen()) {
      placePanel();
    }
    if (prev === null || permission() !== "granted" || inForeground()) {
      return;
    }
    var notes = toNotifications(diff(prev, state), settings);
    for (var i = 0; i < notes.length; i += 1) {
      show(notes[i]);
    }
  }

  // ---------------------------------------------------------------------------
  // 設定面板
  // ---------------------------------------------------------------------------

  function make(tag, className, text) {
    var node = document.createElement(tag);
    if (className) {
      node.className = className;
    }
    if (text !== undefined && text !== null) {
      node.textContent = text;
    }
    return node;
  }

  var panel = make("div", "notify-panel");
  panel.id = "notify-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-labelledby", "notify-panel-title");
  panel.hidden = true;

  var heading = make("h2", "notify-panel-title", t("notify.panel.title"));
  heading.id = "notify-panel-title";
  panel.appendChild(heading);
  panel.appendChild(make("p", "notify-panel-note", t("notify.panel.note")));

  var toggles = [];
  var kindList = make("ul", "notify-kinds");
  for (var k = 0; k < KINDS.length; k += 1) {
    var kind = KINDS[k];
    var item = make("li");
    var label = make("label", "notify-kind");
    var box = make("input");
    box.type = "checkbox";
    box.setAttribute("data-notify-kind", kind);
    var text = make("span", "notify-kind-text");
    text.appendChild(make("span", "notify-kind-name", KIND_LABELS[kind].name));
    text.appendChild(make("span", "notify-kind-desc", KIND_LABELS[kind].desc));
    label.appendChild(box);
    label.appendChild(text);
    item.appendChild(label);
    kindList.appendChild(item);
    toggles.push(box);
  }
  panel.appendChild(kindList);

  // 變更立即生效並寫入本機儲存（spec「通知設定」）。
  kindList.addEventListener("change", function (event) {
    var target = event.target;
    var changed = target && target.getAttribute ? target.getAttribute("data-notify-kind") : null;
    if (changed && Object.prototype.hasOwnProperty.call(DEFAULTS, changed)) {
      settings[changed] = target.checked === true;
      saveSettings();
    }
  });

  var permissionBox = make("div", "notify-permission");
  panel.appendChild(permissionBox);

  function syncToggles() {
    for (var i = 0; i < toggles.length; i += 1) {
      toggles[i].checked = settings[toggles[i].getAttribute("data-notify-kind")] === true;
    }
  }

  // 多個同源視窗（啟動器會開出多個 --app 視窗）：別的視窗改了設定時，瀏覽器對「其他」同源頁面送
  // `storage` 事件。這裡重讀（與載入同一套逐鍵驗證）並同步開著的面板，之後發通知與存檔都用新設定，
  // 才不會用舊設定照發、也不會在本視窗改一個開關時把別的視窗的變更整份蓋掉（5.3 審查 I2）。
  // `key === null` 是整個儲存被清空，同樣重讀（結果為預設值）。
  window.addEventListener("storage", function (event) {
    if (event.key === STORAGE_KEY || event.key === null) {
      settings = loadSettings();
      syncToggles();
    }
  });

  // 權限狀態（spec「通知設定」）：已允許／尚未決定（「允許通知」按鈕，按下才請求）／已封鎖（指引到
  // 瀏覽器網站設定）／不支援。
  function renderPermission() {
    var current = permission();
    var message;
    var button = null;
    if (current === "granted") {
      message = t("notify.permission.granted");
    } else if (current === "denied") {
      message = t("notify.permission.denied");
    } else if (current === "unsupported") {
      message = t("notify.permission.unsupported");
    } else {
      message = t("notify.permission.default");
      button = make("button", "action-button", t("notify.permission.allowButton"));
      button.type = "button";
      button.addEventListener("click", requestPermission);
    }
    var nodes = [make("p", "notify-permission-text", message)];
    if (button !== null) {
      nodes.push(button);
    }
    permissionBox.replaceChildren.apply(permissionBox, nodes);
  }

  var requesting = false;

  function requestPermission() {
    var api = notificationApi();
    if (api === null || requesting) {
      return;
    }
    requesting = true;
    var done = false;
    var finish = function () {
      if (done) {
        return;
      }
      done = true;
      requesting = false;
      var hadFocus = panel.contains(document.activeElement);
      renderPermission();
      // 「允許通知」按鈕被換掉時焦點會掉回 body：留在面板內（第一個開關）。
      if (isOpen() && hadFocus && !panel.contains(document.activeElement)) {
        focusEl(toggles[0]);
      }
    };
    try {
      // 舊版瀏覽器只支援回呼形式；新版回傳 Promise。兩者都接，finish 只做一次。
      var result = api.requestPermission(finish);
      if (result && typeof result.then === "function") {
        result.then(finish, finish);
      }
    } catch (e) {
      finish();
    }
  }

  function focusEl(node) {
    if (!node || typeof node.focus !== "function") {
      return;
    }
    if (window.cockpitFocus && typeof window.cockpitFocus.focus === "function") {
      window.cockpitFocus.focus(node);
    } else {
      node.focus({ preventScroll: true });
    }
  }

  function currentBell() {
    return document.querySelector(BELL_SELECTOR);
  }

  function isOpen() {
    return !panel.hidden;
  }

  // 開關面板的唯一入口：同步當下這顆鈴鐺的 aria-expanded（修正波，3.5 採納項）。開關面板不觸發
  // 整頁重畫，所以要直接改目前的鈴鐺；之後重畫出來的新鈴鐺由 render.js 依 isOpen() 設定。
  function setOpen(open) {
    panel.hidden = !open;
    var bell = currentBell();
    if (bell !== null) {
      bell.setAttribute("aria-expanded", open ? "true" : "false");
    }
  }

  var VIEWPORT_MARGIN = 8;
  var BELL_GAP = 6;

  // 定位（修正波 3.6 M4）：面板是 position: fixed，貼在鈴鐺下方、右緣對齊鈴鐺，並夾在視窗內——上緣
  // 不小於 8px、下緣不超出視窗（面板比視窗高時由 style.css 的 max-height 讓它內部捲動）。寬 <1200 的
  // 版面整頁可捲、鈴鐺可能被捲走，所以：開啟時先把鈴鐺捲進視野（openPanel）；開著時頁面或任何容器
  // 捲動、視窗改變大小、收到新狀態整頁重畫（頂列高度可能變）都重新定位。鈴鐺被捲到視窗上方之外時
  // 面板停在視窗頂端（仍看得到，焦點仍在面板內），捲回來後又貼回鈴鐺下方。選「跟著重新定位」而不選
  // 「一捲動就關閉」：面板內部本身可捲動、背景重畫也可能伴隨捲動事件，捲動即關閉會讓面板在使用者沒有
  // 操作時自己消失（spec「重畫不關閉面板」）。找不到鈴鐺時用 style.css 的預設位置。
  function placePanel() {
    var bell = currentBell();
    if (bell === null) {
      panel.style.top = "";
      panel.style.right = "";
      return;
    }
    var rect = bell.getBoundingClientRect();
    var viewportWidth = document.documentElement.clientWidth;
    var maxTop = window.innerHeight - panel.offsetHeight - VIEWPORT_MARGIN;
    var top = Math.max(VIEWPORT_MARGIN, Math.min(rect.bottom + BELL_GAP, maxTop));
    panel.style.top = Math.round(top) + "px";
    panel.style.right = Math.max(VIEWPORT_MARGIN, Math.round(viewportWidth - rect.right)) + "px";
  }

  function openPanel() {
    settings = normalizeSettings(settings);
    syncToggles();
    renderPermission();
    // 鈴鐺不完全在視窗內（窄版頁面已往下捲）時先捲進來，面板才會出現在鈴鐺下方看得到的地方。
    var bell = currentBell();
    if (bell !== null) {
      var rect = bell.getBoundingClientRect();
      if (rect.top < 0 || rect.bottom > window.innerHeight) {
        bell.scrollIntoView({ block: "nearest", inline: "nearest" });
      }
    }
    setOpen(true);
    placePanel();
    focusEl(toggles[0]);
  }

  // 關閉；焦點原本在面板內時回到（目前的）鈴鐺。
  function closePanel() {
    var hadFocus = panel.contains(document.activeElement);
    setOpen(false);
    if (hadFocus) {
      focusEl(currentBell());
    }
    return hadFocus;
  }

  function togglePanel() {
    if (isOpen()) {
      closePanel();
    } else {
      openPanel();
    }
  }

  // 面板內目前可用 Tab 到達的元素（DOM 順序）。
  function panelFocusables() {
    var all = panel.querySelectorAll("input, button, select, textarea, a[href], [tabindex]");
    var out = [];
    for (var i = 0; i < all.length; i += 1) {
      var node = all[i];
      if (!node.disabled && node.tabIndex >= 0 && node.getClientRects().length > 0) {
        out.push(node);
      }
    }
    return out;
  }

  // Tab 順序（修正波 3.6 M9）：面板是 body 尾端的節點，DOM 順序離鈴鐺很遠（第一個開關按 Shift+Tab
  // 會跳到頁面最後一個元素，最後一個按 Tab 會離開頁面）。不把面板節點搬到鈴鐺旁：鈴鐺在 #app 內、
  // 每次重畫重建，面板必須留在 #app 之外（design D8）。改成在鍵盤上讓面板「接在鈴鐺之後」：
  //   - 面板開著、焦點在鈴鐺時按 Tab：進到面板第一個可聚焦元素。
  //   - 面板第一個可聚焦元素按 Shift+Tab：回到鈴鐺，面板維持開著（可再按 Tab 回來，或按鈴鐺關閉）。
  //   - 面板最後一個可聚焦元素按 Tab：關閉面板、焦點回鈴鐺（同 Esc 的關閉規則），再按 Tab 才離開頂列。
  // 其餘情況（面板中間的元素、焦點不在面板與鈴鐺上、在鈴鐺上按 Shift+Tab）交給瀏覽器預設。
  function handleTab(event) {
    var items = panelFocusables();
    if (items.length === 0) {
      return;
    }
    var active = document.activeElement;
    var bell = currentBell();
    if (bell !== null && active === bell) {
      if (!event.shiftKey) {
        event.preventDefault();
        focusEl(items[0]);
      }
      return;
    }
    if (event.shiftKey && active === items[0] && bell !== null) {
      event.preventDefault();
      focusEl(bell);
    } else if (!event.shiftKey && active === items[items.length - 1]) {
      event.preventDefault();
      closePanel();
    }
  }

  // 鍵盤（掛在 document 的冒泡階段，晚於元素上的處理常式）：
  //   - Esc 關閉（焦點在面板內或其他地方都一樣）。修正波 3.6 M5：事件已被其他處理常式
  //     preventDefault 時不動作——例如 Git Graph 分支篩選 popover 與比較基準各自在元素上處理 Esc，
  //     否則一次 Esc 會同時關掉兩個浮層。關閉後也 preventDefault，讓之後的處理常式知道這次已被用掉。
  //   - Tab／Shift+Tab：見 handleTab。帶 Alt、Ctrl、Meta 的不處理。
  document.addEventListener("keydown", function (event) {
    if (event.defaultPrevented || !isOpen()) {
      return;
    }
    if (event.key === "Escape" || event.key === "Esc") {
      event.preventDefault();
      closePanel();
    } else if (event.key === "Tab" && !event.altKey && !event.ctrlKey && !event.metaKey) {
      handleTab(event);
    }
  });

  // 點面板與鈴鐺以外的位置關閉（design D8：判定排除鈴鐺本身，否則開啟的那次點擊會立刻把面板關掉；
  // 鈴鐺的開關由 actions.js 的委派處理）。這個 listener 掛在 document 的冒泡階段，晚於 #app 上
  // actions.js 的 pointerdown 委派。焦點原本在面板內時要回到鈴鐺，但這次按壓接下來的 mousedown
  // 預設動作會把焦點移到被按的位置（按在不可聚焦的地方就是 body），所以等到 pointerup 才處理：
  // 那時焦點若落在 body（或仍在已隱藏的面板裡）就交給鈴鐺；若按到的是可聚焦的元素（例如 pane 列，
  // 焦點已由該操作接手），不搶走。
  var returnFocusOnPointerUp = false;

  document.addEventListener("pointerdown", function (event) {
    if (!isOpen()) {
      return;
    }
    var target = event.target;
    if (target instanceof Node && panel.contains(target)) {
      return;
    }
    if (target instanceof Element && target.closest('[data-action="notify-settings"]') !== null) {
      return;
    }
    var hadFocus = panel.contains(document.activeElement);
    setOpen(false);
    returnFocusOnPointerUp = hadFocus;
  });

  function settleFocusAfterPointer() {
    if (!returnFocusOnPointerUp) {
      return;
    }
    returnFocusOnPointerUp = false;
    var active = document.activeElement;
    if (!active || active === document.body || panel.contains(active)) {
      focusEl(currentBell());
    }
  }

  document.addEventListener("pointerup", settleFocusAfterPointer);
  document.addEventListener("pointercancel", settleFocusAfterPointer);

  // 開著時跟著視窗大小與捲動重新定位（修正波 3.6 M4；見 placePanel）。捲動用捕獲階段：容器內部的
  // scroll 事件不冒泡，掛在 window 的捕獲階段才收得到。
  function replaceIfOpen() {
    if (isOpen()) {
      placePanel();
    }
  }

  window.addEventListener("resize", replaceIfOpen);
  window.addEventListener("scroll", replaceIfOpen, { capture: true, passive: true });

  syncToggles();
  renderPermission();
  document.body.appendChild(panel);

  window.cockpitNotify = {
    observe: observe,
    togglePanel: togglePanel,
    isOpen: isOpen,
    diff: diff,
    toNotifications: toNotifications,
  };
})();
