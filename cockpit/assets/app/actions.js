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
// 選取（spec live-output「選定一個 pane」；design D8；task 5.3／fix round 1 R15）：
// `ui.selected` 跟 `rebind`／`error` 並列，`uiSnapshot()` 一併交給 render.js。可點的目標不只
// `<button>`——pane 列是 `<div data-action="select-pane" tabindex="0">`，所以委派選取器放寬成
// `[data-action]`（原本是 `button[data-action]`）；`<button>` 本身仍靠瀏覽器原生的 Enter／
// Space → click（detail 0）取得鍵盤可及性，非 `<button>` 的可點元素另外用檔尾的 keydown
// listener 補。改綁模式期間 pane 列整列不可點選（R15，render.js 端不輸出 `data-action`／
// `tabindex`；只留「綁定到這裡」按鈕可點），所以「按『綁定到這裡』不得同時觸發選取」不再需要
// 靠 `closest("[data-action]")` 挑最近的那個來擋——外層那一列這時根本沒有 `data-action`；
// `closest()` 只是保留一般性的事件委派機制，不是這個規則的主要防線。
// `ui.selected` 改變時同步呼叫 `window.liveOutput.select`／由 `window.cockpitActions
// .clearSelected` 只清 `ui.selected`（不呼叫 `liveOutput.clear()`，見該函式註解）。
//
// 選取不是「畫面操作」（G4 fix wave Finding 2／R19）：`perform()` 對 `select-pane`／
// `select-bound-pane`（含鍵盤觸發）在最前面獨立處理、直接 `return`，不落入下面共用的
// `latestOp += 1`／`ui.error = null`。原因：選取只是切換 Live Output 面板看哪個 pane，不是
// spec「畫面操作」定義的那組寫入按鈕；若選取也遞增 `latestOp`，使用者按下「推進」之後立刻去點
// pane 看輸出，那筆寫入稍後才失敗時，`showError()` 會因為 op 已經被選取動作推走而忽略，寫入
// 失敗被悄悄吞掉；若選取也清 `ui.error`，已經顯示的錯誤訊息會被下一次選取清掉，同樣違反 spec
// 「頁面顯示錯誤訊息……直到下一次操作或使用者關閉」。`clearSelected()`（面板「關閉」與「pane
// 已不存在」的回呼）同理，本來就沒有動 `latestOp`／`ui.error`，不需要另外處理。
//
// Project 選取（spec cockpit-dashboard「Project 切換」；design D6；direction-01-visual
// task 3.1）：`ui.selectedProject` 跟 `rebind`／`error`／`selected` 並列存在 `ui` 裡、一併交給
// `uiSnapshot()`。理由跟上一段的 pane 選取完全對應（design D6 明文「選定 Project 不是『畫面
// 操作』」）：`perform()` 對 `select-project` 也在最前面獨立處理、直接 `return`，不遞增
// `latestOp`、不清 `ui.error`，也完全不碰 `ui.rebind`（不會離開改綁模式）。左欄項目是
// `<button data-action="select-project" data-project="...">`，鍵盤可及性靠瀏覽器原生的
// Enter／Space → click（detail 0）即可，不需要像 pane 列那樣另外補 `tabIndex`／`role`／
// keydown（design D6：「左欄項目是 button」）。render.js 的 `renderState()` 用
// `ui.selectedProject` 決定 Factory Floor 畫哪個 Project，找不到（尚未選定過，或選定的
// Project 已不在最新投影中）就用 `state.projects` 的第一個。
//
// 載入順序：index.html 依序載 output.js → render.js → actions.js → channel.js。#app 在
// <body> 內、script 在它之後，這裡執行時一定已存在；`window.liveOutput` 也已經就緒。
//
// 焦點還原的 pointerdown 提示（fix round 1 Finding 1；task focus-fix）：`pointerdown`
// listener 呼叫 `perform()` 前後各呼叫一次 `window.cockpitFocusHint.setPendingTarget`
// （render.js 提供），把「這次真正被按下的目標」交給 render.js 的 `paint()`——多數 action 都
// 會在 `perform()` 內同步觸發 `repaint()`，瀏覽器套用「聚焦被按下元素」這個預設動作的時機不
// 保證早於這次同步重畫，只看 `document.activeElement` 抓不到真正被按下的目標。詳見
// `render.js` `consumePendingFocusIdentity` 上方註解。

(function () {
  "use strict";

  var root = document.getElementById("app");

  var ui = {
    rebind: null, // null | { project, workstream }
    error: null, // null | string
    selected: null, // null | { runtime, paneId }（design D8）
    selectedProject: null, // null | string（design D6；direction-01-visual task 3.1）
  };

  function uiSnapshot() {
    return {
      rebind: ui.rebind === null ? null : { project: ui.rebind.project, workstream: ui.rebind.workstream },
      error: ui.error,
      selected: ui.selected === null ? null : { runtime: ui.selected.runtime, paneId: ui.selected.paneId },
      selectedProject: ui.selectedProject,
    };
  }

  function repaint() {
    if (typeof window.repaint === "function") {
      window.repaint();
    }
  }

  // Live Output 面板自己決定要停止時（使用者按「關閉」、或偵測到 pane 已不存在）回呼這裡
  // （design D8；task 5.2 report「5.3 實作者請在 actions.js 補上 window.cockpitActions
  // .clearSelected」）。刻意**不**呼叫 `window.liveOutput.clear()`：面板該保留的畫面（例如
  // 「pane 已不存在」的過期內容、停止輪詢）output.js 自己已經處理完，這裡再呼叫回去只會把它
  // 蓋掉（面板會被立刻收起），形成互叫。外部（其他程式碼）不應該呼叫這個函式來「取消選取」
  // ——那應該直接改 `ui.selected` 並呼叫 `window.liveOutput.clear()`，目前沒有這樣的路徑
  // （唯一的取消選取入口是面板的「關閉」，已經算在上面）。
  function clearSelected() {
    ui.selected = null;
    repaint();
  }

  // 選定的 Project 已不在最新投影中時，正式改成第一個（spec cockpit-dashboard「Project
  // 切換」：「選定的 Project 已不在最新投影中時改為選定第一個」；design D6；
  // direction-01-visual task 3.1 fix round 1／Codex finding：`render.js` 原本只在
  // `renderState()` 內部 fallback 到第一個 Project 讓畫面正確，卻沒有把這裡的
  // `ui.selectedProject` 一併改掉——若使用者選了 p2、下一份投影暫時沒有 p2（畫面正確
  // fallback 顯示第一個），p2 之後又出現在投影裡時，這裡殘留的舊 ID 會讓畫面在使用者沒有
  // 任何操作的情況下自己跳回 p2，不符合 spec「改為選定第一個」是正式狀態改變、不是暫時顯示
  // fallback 的原意。`render.js` 的 `paint()` 發現目前記錄的選取 ID 不在投影裡時呼叫這裡；
  // 呼叫端已經知道有這件事發生、正在處理當下這次重畫，這裡只改狀態、不额外 `repaint()`
  // （避免同一次重畫觸發第二次重畫；render.js 內部的 fallback 邏輯已經讓這次畫面正確）。
  // 跟 `clearSelected()` 一樣，外部只應該由那個唯一的呼叫端（render.js paint()）呼叫。
  function setSelectedProject(id) {
    ui.selectedProject = id;
  }

  window.cockpitActions = {
    uiSnapshot: uiSnapshot,
    clearSelected: clearSelected,
    setSelectedProject: setSelectedProject,
  };

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

  var TASK_OPS = { advance: true, retreat: true, complete: true, fail: true, clear: true };

  function perform(el) {
    var data = el.dataset;
    var action = data.action;

    if (action === "select-project") {
      // Project 選取不是「畫面操作」（design D6，比照 select-pane／select-bound-pane 的
      // 理由——R19／G4 fix wave Finding 2；direction-01-visual task 3.1）：不遞增
      // latestOp、不清 ui.error、不影響改綁模式（perform() 完全不碰 ui.rebind）。它只切換
      // 左欄哪個 Project 被標示為選定、render.js 的 renderState() 依此決定 Factory Floor
      // 畫哪個 Project——跟「畫面操作」定義的那組寫入按鈕（task/workstream 的 fetch）無關，
      // 若也遞增 latestOp，使用者切換 Project 後若剛好有一筆寫入請求稍後才失敗，
      // showError() 會因為 op 已被這次切換推走而忽略，寫入失敗被悄悄吞掉；若也清
      // ui.error，已顯示的錯誤訊息會被切換 Project 清掉，兩者都違反 spec「畫面操作」
      // 「頁面顯示錯誤訊息……直到下一次操作或使用者關閉」與「Project 切換」「選定 Project
      // 不是『畫面操作』：不得清除最近一次操作的錯誤訊息……不得離開改綁模式」。
      ui.selectedProject = data.project;
      repaint();
      return;
    }

    if (action === "select-pane" || action === "select-bound-pane") {
      // 選取不是「畫面操作」（spec cockpit-dashboard「畫面操作」只列 task／workstream 的寫入
      // 按鈕；design D8；R19）：不遞增 latestOp、不清 ui.error。否則按下「推進」之後立刻去點
      // pane 看輸出，latestOp 會被選取動作往前推，那筆寫入稍後才失敗時 showError() 會因為
      // op 不符而忽略——寫入失敗被選取動作悄悄吞掉；已經顯示的錯誤訊息也會被下一次選取清掉，
      // 兩者都違反 spec「畫面操作」「頁面顯示錯誤訊息……直到下一次操作或使用者關閉」。
      ui.selected = { runtime: data.runtime, paneId: data.pane };
      if (window.liveOutput && typeof window.liveOutput.select === "function") {
        window.liveOutput.select(data.runtime, data.pane);
      }
      repaint();
      return;
    }

    var op = (latestOp += 1);

    // 任何一次「畫面操作」都清掉上一次的錯誤訊息（「直到下一次操作或使用者關閉」）。
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

  // 可點的目標不限 <button>：pane 列是 data-action="select-pane" 的 <div>（design D8；task
  // 5.3）。closest("[data-action]") 一律先命中離事件目標最近的那個帶 data-action 元素——改綁
  // 模式下巢在可點 pane 列裡的「綁定到這裡」按鈕本身也有 data-action，會比外層的 pane 列先
  // 命中，天然滿足「按『綁定到這裡』不得同時觸發選取」，不需要 stopPropagation。
  function actionTarget(event) {
    var node = event.target;
    if (!node || typeof node.closest !== "function") {
      return null;
    }
    var target = node.closest("[data-action]");
    return target !== null && root.contains(target) ? target : null;
  }

  // fix round 1 Finding 1：瀏覽器套用「把焦點移到被按下元素」這個預設動作的時機不保證早於
  // perform() 內同步觸發的 repaint()——render.js 的 paint() 開始時 document.activeElement
  // 可能還不是這次真正被按下的 target（實測：既不是按下前的舊焦點，也還不是 target；見
  // live-output-check.js U 段 RED）。把 target 交給 render.js（若有提供這個 hook），讓這次
  // 同步觸發的重畫優先還原焦點到 target，而不是照 document.activeElement 猜。perform()
  // 呼叫前後各設一次（前設 target、後清成 null）：多數 action 都會在 perform() 內同步
  // repaint（此時 render.js 的 paint() 會自己讀走並清空這個值）；萬一某次委派到的
  // data-action 不被 perform() 任何分支接受、沒有觸發 repaint，這裡的清空也能防止這次沒用到
  // 的目標殘留到下一次（不論是下一次 pointerdown 還是之後由 /ws 推送觸發的）重畫。
  root.addEventListener("pointerdown", function (event) {
    // 只收主要按鍵（滑鼠左鍵、觸控、筆）；右鍵／中鍵不觸發操作。
    if (event.button !== 0) {
      return;
    }
    var target = actionTarget(event);
    if (target !== null) {
      // fix round 1 Finding 1（根因排查後補上）：呼叫 perform() 送出的同步 repaint 換掉
      // #app 整棵樹之後，瀏覽器仍會為這次滑鼠按壓補送相容用的 mousedown／mouseup／click
      // （Pointer Events 相容事件；實測用 focusin／focusout／mousedown 等事件掛
      // document 層級的診斷 listener 抓到：repaint 後補送的那個 mousedown 用「按下當下」
      // 的座標重新對新 DOM 做一次 hit-test，命中的往往不是新按鈕本身而是外層容器
      // （例如 `.task-actions`），這個 mousedown 的預設聚焦動作會把我們剛剛還原好的焦點
      // 又搶走、吹到沒有任何元素聚焦）。Pointer Events 規格允許用 preventDefault() 完全
      // 關掉這組相容事件（連同它們的預設動作）：這裡的 perform() 已經自己完整處理了這次
      // 按壓要做的事，不需要瀏覽器另外補送 mousedown／mouseup／click，關掉它們不會少做
      // 任何事——鍵盤觸發的 Enter／Space（在聚焦的 <button> 上原生轉成的 click、以及非
      // <button> 元素走檔尾 keydown listener）完全是另一條路徑，不受影響。
      event.preventDefault();
      var focusHint = window.cockpitFocusHint;
      if (focusHint && typeof focusHint.setPendingTarget === "function") {
        focusHint.setPendingTarget(target);
      }
      // fix round 2 Finding（task focus-fix2）：perform() 的同步呼叫鏈包含
      // window.liveOutput.select、repaint 與 DOM rendering，任一處拋錯都可能中途中止；原本清除
      // 待還原目標的那行寫在 perform() 之後、沒有 try/finally，例外一拋就被整段跳過，讓這個
      // 待還原目標殘留到下一次重畫（不論觸發原因是什麼，包含完全無關的 /ws 推送），優先蓋掉當下
      // 真正的 document.activeElement，把焦點錯誤跳到早已按下失敗的舊目標上（見
      // live-output-check.js V 段 RED：焦點被跳到已注入例外的 pane 列，不是重畫前真正聚焦的
      // 元素）。改用 try/finally 確保清除一定執行；例外本身不吞，仍照常往外拋（pointerdown
      // 委派原本就沒有包住 perform() 的呼叫，這裡不新增任何 catch）。
      try {
        perform(target);
      } finally {
        if (focusHint && typeof focusHint.setPendingTarget === "function") {
          focusHint.setPendingTarget(null);
        }
      }
    }
  });

  root.addEventListener("click", function (event) {
    // 鍵盤觸發的 click（detail 為 0）才處理；滑鼠 click 已經在 pointerdown 處理過。這條路徑
    // 對 <button> 已經夠：瀏覽器原生把聚焦 <button> 上的 Enter／Space 轉成這種 click。
    if (event.detail === 0) {
      var target = actionTarget(event);
      if (target !== null) {
        perform(target);
      }
    }
  });

  // 鍵盤可及性（brief「pane 列變成可點之後，至少要能用鍵盤觸發」）：pane 列是
  // <div tabindex="0">，瀏覽器不會像 <button> 一樣替它把 Enter／Space 自動轉成 click，這裡
  // 補上。<button> 已經由上面的 click listener 處理，這裡遇到就跳過，避免送兩次。
  root.addEventListener("keydown", function (event) {
    if (event.key !== "Enter" && event.key !== " " && event.key !== "Spacebar") {
      return;
    }
    var target = actionTarget(event);
    if (target === null || target.tagName === "BUTTON") {
      return;
    }
    event.preventDefault();
    perform(target);
  });
})();
