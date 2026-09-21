// render.js：收到整張投影圖就整頁重畫（spec cockpit-dashboard「畫面整頁重畫」「Factory
// Floor」；設計文件 §8.3；change pipeline-projection design D9）。renderState(state, ui) 是
// 純函數：只讀 state／ui、回傳一棵新建的 DOM 子樹、不讀寫任何全域變數或既有 DOM。
// 會碰全域（document、window）的只有檔尾的 paint／window.onState／window.repaint／
// window.onChannel。
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
// 狀態色塊只有 working／blocked／done／idle／unknown 五種 class；任何其他字串（例如未來
// 協定加的新值）一律落在 unknown 的暗灰色塊，並把原字串保留在文字與 title 裡，不會讓整頁
// 壞掉。Factory Floor 的 task 節點另有自己的六種狀態色（running／blocked／ready／pending／
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

  function renderTopbar(state) {
    var topbar = el("div", "topbar");
    topbar.id = "topbar";

    var name = el("span", "app-name", "AI Agent Cockpit");
    name.id = "app-name";
    topbar.appendChild(name);

    // 通道狀態其實是 channel.js 的事，這裡只是給它一個初始外觀：既然收到了一份投影，
    // 代表 WebSocket 當下是通的。之後真的斷線／重連，一律由 window.onChannel 直接找
    // #channel-status 更新，不會再經過這個函數。
    var channelStatus = el("span", "channel-status channel-connected", "connected");
    channelStatus.id = "channel-status";
    topbar.appendChild(channelStatus);

    var version = el("span", "version", "v" + state.version);
    version.id = "version";
    topbar.appendChild(version);

    return topbar;
  }

  function renderConnection(connection) {
    var box = el("div", "connection conn-" + connection.state);
    box.appendChild(el("span", "connection-state", connection.state));

    if (connection.state === "connected") {
      box.appendChild(el("span", "connection-detail", "server " + connection.server_version));
      box.appendChild(el("span", "connection-detail", "protocol " + connection.protocol));
      box.appendChild(
        el("span", "connection-detail", "last snapshot " + connection.last_snapshot_at)
      );
      if (connection.protocol_warning !== null && connection.protocol_warning !== undefined) {
        box.appendChild(el("span", "protocol-warning", connection.protocol_warning));
      }
    } else if (connection.state === "disconnected") {
      box.appendChild(el("span", "connection-detail", "reason: " + connection.reason));
      box.appendChild(
        el("span", "connection-detail", "retry in " + connection.retry_in_secs + "s")
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

    row.appendChild(el("span", "pane-id", pane.id));
    row.appendChild(el("span", "pane-agent", pane.agent === null || pane.agent === undefined ? "shell" : pane.agent));
    row.appendChild(statusBadge(pane.agent_status));
    row.appendChild(el("span", "pane-title", orDash(pane.title)));
    row.appendChild(el("span", "pane-cwd", orDash(pane.cwd)));

    // 改綁模式：connected runtime 中未 exited 的 pane 列才出現「綁定到這裡」（spec「畫面操作」）。
    if (rebinding && runtime.connection.state === "connected" && !pane.exited) {
      row.classList.add("bind-target");
      row.appendChild(
        actionButton("綁定到這裡", { action: "bind-here", runtime: runtime.id, pane: pane.id })
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
    header.appendChild(statusBadge(tab.agent_status));
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

    var header = el("div", "workspace-header");
    header.appendChild(
      el("span", "workspace-label", workspace.label === null || workspace.label === undefined ? workspace.id : workspace.label)
    );
    header.appendChild(el("span", "workspace-number", "#" + workspace.number));
    header.appendChild(statusBadge(workspace.agent_status));
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

  function renderBindingSummary(binding) {
    var wrap = el("span", "ff-binding ff-binding-" + binding.state);
    switch (binding.state) {
      case "bound":
        wrap.appendChild(
          el("span", "ff-binding-text", binding.runtime + " / " + binding.pane_id)
        );
        if (binding.source === "override") {
          wrap.appendChild(el("span", "ff-binding-badge", "改綁"));
        }
        break;
      case "unbound":
        wrap.appendChild(el("span", "ff-binding-text", "未綁定"));
        break;
      case "ambiguous":
        wrap.appendChild(
          el("span", "ff-binding-text", "歧義（" + binding.candidates.length + "）")
        );
        break;
      case "runtime_disconnected":
        wrap.appendChild(el("span", "ff-binding-text", "runtime 未連線"));
        break;
      case "none":
        wrap.appendChild(el("span", "ff-binding-text", "無綁定"));
        break;
      default:
        // 防禦：未知的 binding.state 不該發生（投影只會輸出這五種之一），但沿用整份檔案
        // 「未知值不壞畫面」的原則，顯示原字串而不是丟例外。
        wrap.appendChild(el("span", "ff-binding-text", String(binding.state)));
    }
    return wrap;
  }

  // 節點按鈕顯示規則（spec「畫面操作」）：mark 為 none 且不在最後一個 stage →「推進」；
  // mark 為 none →「Completed」「Failed」；mark 不是 none → 只有「清除標記」。
  function renderTaskActions(project, task) {
    var actions = el("div", "task-actions");
    function add(label, action) {
      actions.appendChild(
        actionButton(label, { action: action, project: project.id, task: task.id })
      );
    }
    if (task.mark === "none") {
      if (task.stage !== project.stages[project.stages.length - 1]) {
        add("推進", "advance");
      }
      add("Completed", "complete");
      add("Failed", "fail");
    } else {
      add("清除標記", "clear");
    }
    return actions;
  }

  function renderTaskNode(project, task) {
    var cls = taskStatusClass(task.status);
    var node = el("div", "task-node " + cls);
    if (cls === "task-status-unknown") {
      node.title = task.status;
    }
    node.appendChild(el("span", "task-title", task.title));
    node.appendChild(el("span", "task-status-label", task.status));
    node.appendChild(renderTaskActions(project, task));
    return node;
  }

  function renderWorkstreamRowHeader(project, workstream) {
    var header = el("div", "ff-row-header");
    // 跟 ff-cell 一樣的 data-workstream 只是結構標記（供測試腳本按 id 定位這一列，兩個不同
    // Project 可能有同名 workstream.name，光用文字找不準），不是互動屬性。
    header.setAttribute("data-workstream", workstream.id);
    header.appendChild(el("span", "ff-ws-name", workstream.name));
    header.appendChild(renderBindingSummary(workstream.binding));

    // 列首操作：「改綁」一律有；binding.source 為 override 時另有「取消改綁」（spec「畫面操作」）。
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
        actionButton("看輸出", {
          action: "select-bound-pane",
          runtime: workstream.binding.runtime,
          pane: workstream.binding.pane_id,
          project: project.id,
          "source-workstream": workstream.id,
        })
      );
    }
    actions.appendChild(
      actionButton("改綁", { action: "rebind", project: project.id, workstream: workstream.id })
    );
    if (workstream.binding.source === "override") {
      actions.appendChild(
        actionButton("取消改綁", {
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

  function renderFactoryFloor(project) {
    var grid = el("div", "factory-floor");
    // 欄數隨這個 Project 的 stages 數量而定，交由 JS 算出（style.css 只定義固定樣式）。
    grid.style.gridTemplateColumns =
      "minmax(160px, auto) repeat(" + project.stages.length + ", minmax(140px, 1fr))";

    grid.appendChild(el("div", "ff-corner"));
    for (var s = 0; s < project.stages.length; s += 1) {
      grid.appendChild(el("div", "ff-stage-header", project.stages[s]));
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

  function renderProject(project) {
    var section = el("div", "project");
    section.setAttribute("data-project", project.id);

    var header = el("div", "project-header");
    header.appendChild(el("h2", "project-name", project.name));
    if (project.warnings.length > 0) {
      var warnings = el("ul", "project-warnings");
      for (var i = 0; i < project.warnings.length; i += 1) {
        warnings.appendChild(el("li", "project-warning", project.warnings[i]));
      }
      header.appendChild(warnings);
    }
    section.appendChild(header);

    section.appendChild(renderFactoryFloor(project));

    return section;
  }

  function renderRuntimeCard(runtime, rebinding, selected) {
    var card = el("div", "runtime-card");

    var header = el("div", "runtime-header");
    header.appendChild(el("span", "runtime-id", runtime.id));
    header.appendChild(el("span", "runtime-endpoint", runtime.endpoint));
    card.appendChild(header);

    card.appendChild(renderConnection(runtime.connection));

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

  function renderRecentEvents(events) {
    var section = el("div", "recent-events");
    section.appendChild(el("h2", "recent-events-title", "最近事件"));

    var list = el("ul", "recent-events-list");
    var limited = events.slice(0, 50);
    for (var i = 0; i < limited.length; i += 1) {
      var event = limited[i];
      var item = el("li", "event-row");
      item.appendChild(el("span", "event-at", event.at));
      item.appendChild(el("span", "event-runtime", event.runtime));
      item.appendChild(el("span", "event-kind", event.kind));
      item.appendChild(el("span", "event-subject", eventSubject(event)));
      item.appendChild(el("span", "event-detail", event.detail));
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
        "改綁模式：為 " + projectName + " / " + workstreamName + " 選一個 pane，按該列的「綁定到這裡」"
      )
    );
    banner.appendChild(actionButton("取消", { action: "rebind-cancel" }));
    return banner;
  }

  function renderErrorBanner(message) {
    var banner = el("div", "action-banner error-banner");
    banner.setAttribute("role", "alert");
    banner.appendChild(el("span", "action-banner-text", message));
    banner.appendChild(actionButton("關閉", { action: "error-dismiss" }));
    return banner;
  }

  // 純函數：state、選填的 ui → 一棵新建的 DOM 子樹。不讀寫 document 上既有的節點、不留任何
  // 全域狀態。
  function renderState(state, ui) {
    var rebind = ui && ui.rebind ? ui.rebind : null;
    var error = ui && ui.error ? ui.error : null;
    var selected = ui && ui.selected ? ui.selected : null;

    var page = el("div", "page");
    page.appendChild(renderTopbar(state));

    if (error !== null) {
      page.appendChild(renderErrorBanner(error));
    }
    if (rebind !== null) {
      page.appendChild(renderRebindBanner(state, rebind));
    }

    // 每個 Project 一塊 Factory Floor，依 state.projects 順序上下排列，畫在 runtime 卡之前
    // （spec 「Factory Floor」）。
    var projects = el("div", "projects");
    for (var p = 0; p < state.projects.length; p += 1) {
      projects.appendChild(renderProject(state.projects[p]));
    }
    page.appendChild(projects);

    var cards = el("div", "runtime-cards");
    for (var i = 0; i < state.runtimes.length; i += 1) {
      cards.appendChild(renderRuntimeCard(state.runtimes[i], rebind !== null, selected));
    }
    page.appendChild(cards);

    page.appendChild(renderRecentEvents(state.recent_events));

    return page;
  }

  // Live Output（spec live-output「選定一個 pane」；design D8；task 5.3）：交出目前投影裡還
  // 存在的所有 pane（跨 runtime、含 exited——exited 只是不可選，不是不存在），讓 output.js
  // 判斷被選定的 pane 是否已經從投影裡消失。同一個 pane id 可能出現在不同 runtime，所以一定
  // 要帶 runtime。
  function collectKnownPanes(state) {
    var panes = [];
    for (var r = 0; r < state.runtimes.length; r += 1) {
      var runtime = state.runtimes[r];
      for (var w = 0; w < runtime.workspaces.length; w += 1) {
        var workspace = runtime.workspaces[w];
        for (var t = 0; t < workspace.tabs.length; t += 1) {
          var tab = workspace.tabs[t];
          for (var p = 0; p < tab.panes.length; p += 1) {
            panes.push({ runtime: runtime.id, paneId: tab.panes[p].id });
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
  // 「推進」「Completed」「Failed」「清除標記」（`advance`／`complete`／`fail`／`clear`，
  // `data-project`／`data-task`）、workstream 列首「改綁」（`rebind`，
  // `data-project`／`data-workstream`）、「取消改綁」（`override-clear`，同兩個）、改綁提示的
  // 「取消」（`rebind-cancel`，只有 `data-action`）、錯誤訊息的「關閉」
  // （`error-dismiss`，只有 `data-action`）。沒有找到不帶 `data-action` 的可聚焦元素——若之後
  // 新增這種元素，下面的身分規則需要重新檢討。Live Output 面板（`#output`）不在 `#app`
  // 底下、完全不受這裡影響，本來就不需要焦點還原（design D8）。
  //
  // 身分＝`data-action` 加上該元素當下**全部**的 `data-*` 屬性（不逐一列名，直接比對整個
  // `dataset`）：同一個 `data-action` 底下的 data-* 組合是固定的，全量比對比手動列舉哪些欄位
  // 「足以指認」更不容易漏掉、也更不容易在之後新增 data-* 屬性時悄悄變得不準。找同一個對象時
  // 逐一比對 `dataset`（不組 CSS 屬性選擇器字串）——pane id 含冒號，組字串需要正確跳脫（見
  // brief），逐一比對天然沒有這個問題。
  function identityFromElement(el) {
    if (!el || !el.dataset || !el.dataset.action) {
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

  window.cockpitFocusHint = { setPendingTarget: setPendingFocusTarget };

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
    var candidates = appEl.querySelectorAll("[data-action]");
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
  function restoreFocus(appEl, identity) {
    var target = findByFocusIdentity(appEl, identity);
    if (target !== null && typeof target.focus === "function") {
      target.focus({ preventScroll: true });
    }
  }

  function paint() {
    if (latestState === null) {
      return;
    }
    var ui =
      window.cockpitActions && typeof window.cockpitActions.uiSnapshot === "function"
        ? window.cockpitActions.uiSnapshot()
        : undefined;
    var appEl = document.getElementById("app");
    // fix round 1 Finding 1：pointerdown 觸發的同步重畫優先用「待還原目標」（見上方
    // consumePendingFocusIdentity 註解），沒有才照舊看 document.activeElement。
    var focusIdentity = consumePendingFocusIdentity() || captureFocusIdentity(appEl);
    appEl.replaceChildren(renderState(latestState, ui));
    // #output 不在 #app 底下、不被上面這行換掉（design D8）；每次重畫後仍要交出最新的 pane
    // 集合，讓 output.js 判斷被選的 pane 是否已經消失。
    if (window.liveOutput && typeof window.liveOutput.setKnownPanes === "function") {
      window.liveOutput.setKnownPanes(collectKnownPanes(latestState));
    }
    restoreFocus(appEl, focusIdentity);
  }

  window.onState = function (state) {
    latestState = state;
    paint();
  };

  // 給 actions.js：UI 狀態改變後以最新投影重畫（沒有收過投影時什麼都不做）。
  window.repaint = paint;

  window.onChannel = function (status) {
    var badge = document.getElementById("channel-status");
    if (!badge) {
      return;
    }
    badge.textContent = status;
    badge.className = "channel-status channel-" + status;
  };
})();
