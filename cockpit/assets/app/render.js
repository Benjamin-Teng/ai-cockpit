// render.js：收到整張投影圖就整頁重畫（spec cockpit-dashboard「畫面整頁重畫」；設計文件
// §8.3）。renderState(state) 是純函數：只讀 state、回傳一棵新建的 DOM 子樹、不讀寫任何全域
// 變數或既有 DOM。window.onState／window.onChannel 是唯二會碰全域（document）的地方。
//
// 狀態色塊只有 working／blocked／done／idle／unknown 五種 class；任何其他字串（例如未來
// 協定加的新值）一律落在 unknown 的暗灰色塊，並把原字串保留在文字與 title 裡，不會讓整頁
// 壞掉。所有文字一律用 textContent 寫入，不用 innerHTML，避免把資料當成標記解析。

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

  function renderPane(pane) {
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

    return row;
  }

  function renderTab(tab) {
    var box = el("div", "tab");
    if (tab.focused) {
      box.classList.add("focused");
    }

    var header = el("div", "tab-header");
    header.appendChild(el("span", "tab-number", "tab " + tab.number));
    header.appendChild(statusBadge(tab.agent_status));
    box.appendChild(header);

    for (var i = 0; i < tab.panes.length; i += 1) {
      box.appendChild(renderPane(tab.panes[i]));
    }

    return box;
  }

  function renderWorkspace(workspace) {
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
      box.appendChild(renderTab(workspace.tabs[i]));
    }

    return box;
  }

  function renderRuntimeCard(runtime) {
    var card = el("div", "runtime-card");

    var header = el("div", "runtime-header");
    header.appendChild(el("span", "runtime-id", runtime.id));
    header.appendChild(el("span", "runtime-endpoint", runtime.endpoint));
    card.appendChild(header);

    card.appendChild(renderConnection(runtime.connection));

    for (var i = 0; i < runtime.workspaces.length; i += 1) {
      card.appendChild(renderWorkspace(runtime.workspaces[i]));
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

  // 純函數：state → 一棵新建的 DOM 子樹。不讀寫 document 上既有的節點、不留任何全域狀態。
  function renderState(state) {
    var page = el("div", "page");
    page.appendChild(renderTopbar(state));

    var cards = el("div", "runtime-cards");
    for (var i = 0; i < state.runtimes.length; i += 1) {
      cards.appendChild(renderRuntimeCard(state.runtimes[i]));
    }
    page.appendChild(cards);

    page.appendChild(renderRecentEvents(state.recent_events));

    return page;
  }

  window.onState = function (state) {
    var root = document.getElementById("app");
    root.replaceChildren(renderState(state));
  };

  window.onChannel = function (status) {
    var badge = document.getElementById("channel-status");
    if (!badge) {
      return;
    }
    badge.textContent = status;
    badge.className = "channel-status channel-" + status;
  };
})();
