// render.js：收到整張投影圖就整頁重畫（spec cockpit-dashboard「畫面整頁重畫」「Factory
// Floor」；設計文件 §8.3；change pipeline-projection design D9）。renderState(state, ui) 是
// 純函數：只讀 state／ui、回傳一棵新建的 DOM 子樹、不讀寫任何全域變數或既有 DOM。
// 會碰全域（document、window）的只有檔尾的 paint／window.onState／window.repaint／
// window.onChannel。
//
// ui 是選填的第二參數，缺省＝無改綁模式、無錯誤訊息（形狀見 actions.js 的 uiSnapshot：
// `{ rebind: null | { project, workstream }, error: null | string }`）。畫面操作的按鈕（spec
// 「畫面操作」；task 5.3）在這裡只輸出 `<button>` 與 `data-action`／`data-project`／`data-task`／
// `data-workstream`／`data-runtime`／`data-pane` 屬性，**不綁任何 listener**——事件委派、
// fetch 與 UI 狀態都在 actions.js（design D9）。
//
// 狀態色塊只有 working／blocked／done／idle／unknown 五種 class；任何其他字串（例如未來
// 協定加的新值）一律落在 unknown 的暗灰色塊，並把原字串保留在文字與 title 裡，不會讓整頁
// 壞掉。Factory Floor 的 task 節點另有自己的六種狀態色（running／blocked／ready／pending／
// failed／completed），未知字串同樣落在暗灰、不會壞掉整頁。所有文字一律用 textContent 寫入，
// 不用 innerHTML，避免把資料當成標記解析。

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

  function renderPane(pane, runtime, rebinding) {
    var row = el("div", "pane-row");
    if (pane.exited) {
      row.classList.add("exited");
    }
    if (pane.focused) {
      row.classList.add("focused");
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

  function renderTab(tab, runtime, rebinding) {
    var box = el("div", "tab");
    if (tab.focused) {
      box.classList.add("focused");
    }

    var header = el("div", "tab-header");
    header.appendChild(el("span", "tab-number", "tab " + tab.number));
    header.appendChild(statusBadge(tab.agent_status));
    box.appendChild(header);

    for (var i = 0; i < tab.panes.length; i += 1) {
      box.appendChild(renderPane(tab.panes[i], runtime, rebinding));
    }

    return box;
  }

  function renderWorkspace(workspace, runtime, rebinding) {
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
      box.appendChild(renderTab(workspace.tabs[i], runtime, rebinding));
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

  function renderRuntimeCard(runtime, rebinding) {
    var card = el("div", "runtime-card");

    var header = el("div", "runtime-header");
    header.appendChild(el("span", "runtime-id", runtime.id));
    header.appendChild(el("span", "runtime-endpoint", runtime.endpoint));
    card.appendChild(header);

    card.appendChild(renderConnection(runtime.connection));

    for (var i = 0; i < runtime.workspaces.length; i += 1) {
      card.appendChild(renderWorkspace(runtime.workspaces[i], runtime, rebinding));
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
      cards.appendChild(renderRuntimeCard(state.runtimes[i], rebind !== null));
    }
    page.appendChild(cards);

    page.appendChild(renderRecentEvents(state.recent_events));

    return page;
  }

  // 最近一份投影：actions.js 改了 UI 狀態（進出改綁模式、錯誤訊息）後，要用「最新投影＋新
  // UI 狀態」重畫，不必等下一次推送。
  var latestState = null;

  function paint() {
    if (latestState === null) {
      return;
    }
    var ui =
      window.cockpitActions && typeof window.cockpitActions.uiSnapshot === "function"
        ? window.cockpitActions.uiSnapshot()
        : undefined;
    document.getElementById("app").replaceChildren(renderState(latestState, ui));
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
