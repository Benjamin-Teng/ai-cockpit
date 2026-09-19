// actions.js：畫面操作（spec cockpit-dashboard「畫面操作」；change pipeline-projection design D9）。
//
// 這個檔案持有 UI 狀態——改綁模式的目標 workstream、最近一次操作的錯誤訊息——並負責把按鈕
// 送成 pipeline-progress 的寫入端點 fetch。render.js 只輸出帶 data-* 屬性的按鈕、不綁
// listener；channel.js 只管通道。三者的銜接：
//   - render.js 每次重畫時呼叫 window.cockpitActions.uiSnapshot() 取目前 UI 狀態，所以整頁
//     重畫不會清掉改綁模式與錯誤訊息。
//   - 這裡改了 UI 狀態後呼叫 window.repaint()，用最新一份投影立刻重畫。
//   - 操作成功後不自行修改投影，一律等 /ws 推送的新投影重畫。
//
// 事件委派在 #app 根節點上，以 pointerdown 觸發：整頁重畫可能發生在 mousedown 與 mouseup
// 之間，兩者落在不同 DOM 元素時瀏覽器不送 click，按鈕會「按了沒反應」。鍵盤操作（Enter／
// Space）不會產生 pointerdown，另外收 event.detail === 0 的 click；滑鼠產生的 click
// detail ≥ 1，一律忽略，避免同一次按壓送兩次。
//
// 載入順序：index.html 依序載 render.js → actions.js → channel.js。#app 在 <body> 內、
// script 在它之後，這裡執行時一定已存在。

(function () {
  "use strict";

  var root = document.getElementById("app");

  var ui = {
    rebind: null, // null | { project, workstream }
    error: null, // null | string
  };

  function uiSnapshot() {
    return {
      rebind: ui.rebind === null ? null : { project: ui.rebind.project, workstream: ui.rebind.workstream },
      error: ui.error,
    };
  }

  window.cockpitActions = { uiSnapshot: uiSnapshot };

  function repaint() {
    if (typeof window.repaint === "function") {
      window.repaint();
    }
  }

  function seg(value) {
    return encodeURIComponent(value);
  }

  // 每次操作（任何按鈕）配一個遞增序號。錯誤訊息只記「最近一次操作」的：慢的舊請求在較新
  // 的操作之後才失敗時，它的錯誤已經過期，不得蓋掉畫面（task 5.3 fix round 1）。
  var latestOp = 0;

  // 送出一個寫入請求。2xx 呼叫 onSuccess（若有）；非 2xx 或請求失敗、且 op 仍是最近一次操作時，
  // 把錯誤訊息放進 UI 狀態，直到下一次操作或使用者按「關閉」（spec「畫面操作」）。
  function send(op, method, url, body, onSuccess) {
    var init = { method: method };
    if (body !== undefined) {
      init.headers = { "Content-Type": "application/json" };
      init.body = JSON.stringify(body);
    }
    var label = method + " " + url;
    fetch(url, init).then(
      function (response) {
        if (response.ok) {
          if (onSuccess) {
            onSuccess();
          }
          return undefined;
        }
        return response.text().then(function (text) {
          var reason = null;
          try {
            var parsed = JSON.parse(text);
            if (parsed && typeof parsed.error === "string") {
              reason = parsed.error;
            }
          } catch (e) {
            reason = null;
          }
          if (reason === null) {
            reason = text === "" ? response.statusText : text;
          }
          showError(op, "操作失敗（HTTP " + response.status + "，" + label + "）：" + reason);
        });
      },
      function (err) {
        showError(op, "操作失敗（請求沒有完成，" + label + "）：" + (err && err.message ? err.message : err));
      }
    );
  }

  function showError(op, message) {
    if (op !== latestOp) {
      return;
    }
    ui.error = message;
    repaint();
  }

  function taskUrl(data, op) {
    return "/api/projects/" + seg(data.project) + "/tasks/" + seg(data.task) + "/" + op;
  }

  function overrideUrl(project, workstream) {
    return "/api/projects/" + seg(project) + "/workstreams/" + seg(workstream) + "/override";
  }

  var TASK_OPS = { advance: true, complete: true, fail: true, clear: true };

  function perform(button) {
    var data = button.dataset;
    var action = data.action;
    var op = (latestOp += 1);

    // 任何一次操作都清掉上一次的錯誤訊息（「直到下一次操作或使用者關閉」）。
    ui.error = null;

    if (TASK_OPS[action] === true) {
      send(op, "POST", taskUrl(data, action));
    } else if (action === "rebind") {
      ui.rebind = { project: data.project, workstream: data.workstream };
    } else if (action === "rebind-cancel") {
      ui.rebind = null;
    } else if (action === "bind-here") {
      var target = ui.rebind;
      if (target !== null) {
        send(
          op,
          "PUT",
          overrideUrl(target.project, target.workstream),
          { runtime: data.runtime, pane_id: data.pane },
          function () {
            // 成功才離開改綁模式；若期間使用者已改選別的 workstream，不去動新的模式。
            if (ui.rebind === target) {
              ui.rebind = null;
              repaint();
            }
          }
        );
      }
    } else if (action === "override-clear") {
      send(op, "DELETE", overrideUrl(data.project, data.workstream));
    } else if (action !== "error-dismiss") {
      return;
    }
    repaint();
  }

  function actionTarget(event) {
    var node = event.target;
    if (!node || typeof node.closest !== "function") {
      return null;
    }
    var button = node.closest("button[data-action]");
    return button !== null && root.contains(button) ? button : null;
  }

  root.addEventListener("pointerdown", function (event) {
    // 只收主要按鍵（滑鼠左鍵、觸控、筆）；右鍵／中鍵不觸發操作。
    if (event.button !== 0) {
      return;
    }
    var button = actionTarget(event);
    if (button !== null) {
      perform(button);
    }
  });

  root.addEventListener("click", function (event) {
    // 鍵盤觸發的 click（detail 為 0）才處理；滑鼠 click 已經在 pointerdown 處理過。
    if (event.detail === 0) {
      var button = actionTarget(event);
      if (button !== null) {
        perform(button);
      }
    }
  });
})();
