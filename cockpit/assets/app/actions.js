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

  // 介面文字經字典（i18n.js，ui-language task 2.1）：錯誤訊息是整句範本加具名佔位符，不拼接。
  var t = window.cockpitI18n.t;
  var tMsg = window.cockpitI18n.tMsg;

  var root = document.getElementById("app");

  var ui = {
    rebind: null, // null | { project, workstream }
    error: null, // null | string
    selected: null, // null | { runtime, paneId }（design D8）
    selectedProject: null, // null | string（design D6；direction-01-visual task 3.1）
    // 加入成功、等待含它的投影到達後自動選定的 Project id（repo-projects task 5.1）。只存在這裡（不在 DOM 上），
    // 整頁重畫不會丟掉；render.js 的 paint() 每次重畫前呼叫 applyPendingProjectSelection() 判斷。
    pendingProjectSelection: null, // null | string
    // 「加入」的進行狀態（repo-projects task 5.1 fix round 1；審查 Important 1）：repo key → "sending"（請求進行中）或
    // "added"（已回 201、投影的偵測區還列著它）。有標記的 repo 的「加入」呈現停用（aria-disabled，render.js），再按
    // 直接略過——連點兩下不會送出第二筆、被 409 repo_already_added 拒絕而與成功並存。只存在這裡（不在 DOM），整頁
    // 重畫不會丟掉。失敗即刪；"added" 由 pruneAddingRepos() 在投影的偵測區不再列著它時刪。
    addingRepos: {},
    // 開著「⋯」選單的 Repo Project id（repo-projects task 5.2）。只存在這裡（不在 DOM），整頁重畫不會丟；render.js 依它
    // 畫選單。開關選單不是「畫面操作」（不遞增 latestOp、不清錯誤）——真正的寫入是對話框送出的那一次。
    projectMenu: null, // null | string
  };

  function uiSnapshot() {
    return {
      rebind: ui.rebind === null ? null : { project: ui.rebind.project, workstream: ui.rebind.workstream },
      error: ui.error,
      selected: ui.selected === null ? null : { runtime: ui.selected.runtime, paneId: ui.selected.paneId },
      selectedProject: ui.selectedProject,
      addingRepos: Object.assign({}, ui.addingRepos),
      projectMenu: ui.projectMenu,
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

  // 最新一份投影中是否有這個 pane、且它的列可以點選（runtime id ＋ pane id；render.js 的
  // window.cockpitLatestState）。exited 的 pane 不算：render.js 不給 exited 的 pane 列 data-action
  // （spec live-output「選定一個 pane」：exited 的 pane 不可選），點通知要等同點選 pane 列，規則一致
  // （修正波 3.6 M3）。
  function selectablePaneInLatestState(runtime, paneId) {
    var state =
      typeof window.cockpitLatestState === "function" ? window.cockpitLatestState() : null;
    if (!state || !Array.isArray(state.runtimes)) {
      return false;
    }
    for (var r = 0; r < state.runtimes.length; r += 1) {
      var rt = state.runtimes[r];
      if (!rt || rt.id !== runtime || !Array.isArray(rt.workspaces)) {
        continue;
      }
      for (var w = 0; w < rt.workspaces.length; w += 1) {
        var tabs = rt.workspaces[w] && Array.isArray(rt.workspaces[w].tabs) ? rt.workspaces[w].tabs : [];
        for (var ti = 0; ti < tabs.length; ti += 1) {
          var panes = tabs[ti] && Array.isArray(tabs[ti].panes) ? tabs[ti].panes : [];
          for (var p = 0; p < panes.length; p += 1) {
            if (panes[p] && panes[p].id === paneId) {
              return !panes[p].exited;
            }
          }
        }
      }
    }
    return false;
  }

  // 點 pane 通知時選定該 pane（spec desktop-notifications「通知呈現」：效果等同點選該 pane 列；
  // design D7；desktop-launch-notify task 3.3）：走跟 perform() 的 select-pane 相同的路徑——設定
  // ui.selected、呼叫 liveOutput.select、重畫（右欄選定標示由 ui.selected 決定，只呼叫
  // liveOutput.select 不夠）。改綁模式期間（pane 列本來就不可點選）、pane 已不在最新投影中、或 pane
  // 已 exited（列不可點）時不做事、回 false。同 select-pane：不遞增 latestOp、不清 ui.error。
  //
  // 重畫後把新的選定列捲進視野（block／inline 都 nearest；修正波 3.6 M8）：用滑鼠點 pane 列時那一列
  // 本來就在眼前，點通知時它可能在右欄捲動容器外或窄版頁面的別處，不捲的話使用者帶到前景後看不到選定
  // 標示。只捲 pane 列、不另外去捲 Live Output——點 pane 列本身也不會捲動 Live Output。不移動焦點。
  function selectPane(runtime, paneId) {
    if (ui.rebind !== null || !selectablePaneInLatestState(runtime, paneId)) {
      return false;
    }
    choosePane(runtime, paneId);
    repaint();
    scrollPaneRowIntoView(runtime, paneId);
    return true;
  }

  // 選定一個 pane 的共同流程（pane 列 select-pane、「看輸出」select-bound-pane、點 pane 通知、選定 Project 時自動選
  // pane 都走這裡）：設定 ui.selected、呼叫 liveOutput.select（它再通知 files.js／git.js 換根目錄、分頁區切到 Live
  // Output）。只改狀態、不 repaint()——呼叫端自己重畫（或正在重畫，見 applyPendingProjectSelection）。
  function choosePane(runtime, paneId) {
    ui.selected = { runtime: runtime, paneId: paneId };
    if (window.liveOutput && typeof window.liveOutput.select === "function") {
      window.liveOutput.select(runtime, paneId);
    }
  }

  function selectedPaneRow(runtime, paneId) {
    var rows = root.querySelectorAll(".pane-row.selected");
    for (var i = 0; i < rows.length; i += 1) {
      if (rows[i].getAttribute("data-runtime") === runtime && rows[i].getAttribute("data-pane") === paneId) {
        return rows[i];
      }
    }
    return null;
  }

  // 點 pane 通知用（selectPane）：把右欄 runtime 卡片中 runtime＋pane 那一列捲進視野（block／inline 都 nearest）。
  // scrollIntoView 會捲動所有可捲動的祖先、含整頁——點通知時那一列可能在頁面別處，捲整頁是刻意的。只捲動、不移動焦點。
  function scrollPaneRowIntoView(runtime, paneId) {
    var row = selectedPaneRow(runtime, paneId);
    if (row !== null) {
      row.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
  }

  // 選定 Project 時用（project-select-pane fix round 1 I1）：只捲右欄自己的捲動容器 `.runtime-cards`，**絕不捲整頁**。
  // 整頁捲動的版面（寬 <760、760–1199、≥1200 但高 <720，見 style.css「四種情形」）若用 scrollIntoView，點 Project 會把
  // 整頁跳到 runtime 卡片、Factory Floor 被捲出畫面。只有 `.runtime-cards` 是有界的捲動容器（內容高於可視高度、
  // overflow-y 為 auto／scroll）時才捲，自己算 scrollTop（nearest 語意：列在上方外就讓列頂貼齊可視區頂，在下方外就讓列底
  // 貼齊可視區底，已在可視區內不動）。不移動焦點。
  function scrollPaneRowWithinRuntimeCards(runtime, paneId) {
    var box = root.querySelector('[data-region="runtimes"] > .runtime-cards');
    var row = selectedPaneRow(runtime, paneId);
    if (box === null || row === null || box.scrollHeight <= box.clientHeight) {
      return;
    }
    var overflowY = window.getComputedStyle(box).overflowY;
    if (overflowY !== "auto" && overflowY !== "scroll") {
      return;
    }
    var boxRect = box.getBoundingClientRect();
    var rowRect = row.getBoundingClientRect();
    var viewTop = boxRect.top + box.clientTop;
    var viewBottom = viewTop + box.clientHeight;
    if (rowRect.top < viewTop) {
      box.scrollTop -= viewTop - rowRect.top;
    } else if (rowRect.bottom > viewBottom) {
      // 列比可視區還高時改讓列頂貼齊，避免列頂被捲出去。
      box.scrollTop += Math.min(rowRect.bottom - viewBottom, rowRect.top - viewTop);
    }
  }

  // 選定 Project 時要一併選定的 pane（spec cockpit-dashboard「Project 切換」；project-select-pane task 1.1）：
  // ① `binding.state` 為 `bound` 且 `binding.agent_status` 為 `"working"` 的工作線，依畫面順序（`project.workstreams`
  // 的順序，render.js 依此畫 Factory Floor 的列）取第一條；② 否則第一條 bound；③ 都沒有回 null（不改變目前選定的 pane）。
  // 只讀投影欄位，不複算後端的綁定規則。回傳 { runtime, paneId } 或 null。
  function paneForProject(project) {
    var workstreams = project && Array.isArray(project.workstreams) ? project.workstreams : [];
    var firstBound = null;
    for (var i = 0; i < workstreams.length; i += 1) {
      var binding = workstreams[i] ? workstreams[i].binding : null;
      if (!binding || binding.state !== "bound") {
        continue;
      }
      var pane = { runtime: binding.runtime, paneId: binding.pane_id };
      if (binding.agent_status === "working") {
        return pane;
      }
      if (firstBound === null) {
        firstBound = pane;
      }
    }
    return firstBound;
  }

  // 因使用者選定 Project（點選、鍵盤、加入後自動選定）而選定它的 pane：效果等同按該工作線的「看輸出」（同一個
  // choosePane()），並記下「下次重畫結束後把該 pane 列捲進視野」——捲動要等新畫面的列存在才能做，由 render.js 的
  // paint() 在最後呼叫 flushPaneRowScroll()。改綁模式中不做（spec：改綁模式下 pane 列用於指定改綁目標，不是選定 pane）；
  // 沒有可挑的工作線時不動。頁面載入的預設 Project 與「選定的 Project 消失改選第一個」（setSelectedProject()）不經過這裡。
  // 只改狀態、不 repaint()。
  var paneRowScrollPending = false;
  function selectPaneForProject(project) {
    if (ui.rebind !== null) {
      return;
    }
    var pane = paneForProject(project);
    if (pane === null) {
      return;
    }
    if (ui.selected !== null && ui.selected.runtime === pane.runtime && ui.selected.paneId === pane.paneId) {
      // fix round 1 M1：挑到的就是目前選定的 pane（例如兩個 Project 綁同一個 pane）時不再呼叫 liveOutput.select()——
      // 它會清空 Live Output 的內容與貼底狀態、重新輪詢。只讓分頁區切到 Live Output：files.js 的 paneSelected() 對同一個
      // pane 不重查根目錄、不清檔案樹，只切分頁（切到 Live Output 時 output.js 經 tabShown() 還原貼底狀態）；git.js 的選取
      // 本來就沒變，不必通知。只在選定 Project 這條路徑這樣做：點 pane 列、「看輸出」、點通知的既有行為不變（非目標）。
      if (window.cockpitFiles && typeof window.cockpitFiles.paneSelected === "function") {
        window.cockpitFiles.paneSelected(pane.runtime, pane.paneId);
      }
    } else {
      choosePane(pane.runtime, pane.paneId);
    }
    paneRowScrollPending = true;
  }

  // render.js paint() 每次重畫結束時呼叫：有待捲動（selectPaneForProject 剛選定 pane）就在右欄捲動容器內把選定的 pane
  // 列捲進視野（不捲整頁，見 scrollPaneRowWithinRuntimeCards）。狀態存模組變數（整頁重畫會丟掉只存在 DOM 上的狀態）；
  // 捲動在重畫還原捲動位置之後才做，不會被蓋回去。
  function flushPaneRowScroll() {
    if (!paneRowScrollPending) {
      return;
    }
    paneRowScrollPending = false;
    if (ui.selected !== null) {
      scrollPaneRowWithinRuntimeCards(ui.selected.runtime, ui.selected.paneId);
    }
  }

  function projectInLatestState(id) {
    var state = typeof window.cockpitLatestState === "function" ? window.cockpitLatestState() : null;
    var projects = state && Array.isArray(state.projects) ? state.projects : [];
    for (var i = 0; i < projects.length; i += 1) {
      if (projects[i] && projects[i].id === id) {
        return projects[i];
      }
    }
    return null;
  }

  // 加入 Repo Project 成功後的自動選定（spec cockpit-dashboard「Project 切換」：加入成功——回 201 與新 Project 的
  // `id`——後，畫面在含該 `id` 的投影到達時自動選定這個新 Project，只選取一次，之後使用者可自由切換；repo-projects
  // task 5.1）。render.js 的 paint() 在每次重畫前呼叫，交出最新投影：待選定的 id 已在 `projects` 裡就改成選定它並清掉
  // 待選定（之後的投影不會再把選取拉回來），回傳該 id；否則回傳 null、待選定保留到下一份投影。投影比 201 回應先到時，
  // 回應到達後的那次重畫（見 add-repo 的成功回呼）就會選定。跟 setSelectedProject() 一樣只改狀態、不另外 repaint()
  // （呼叫端正在重畫）。
  function applyPendingProjectSelection(state) {
    var id = ui.pendingProjectSelection;
    if (id === null || !state || !Array.isArray(state.projects)) {
      return null;
    }
    for (var i = 0; i < state.projects.length; i += 1) {
      if (state.projects[i] && state.projects[i].id === id) {
        ui.selectedProject = id;
        ui.pendingProjectSelection = null;
        // project-select-pane task 1.1：加入後自動選定新 Project 也算使用者選定，一併選定它的 pane。
        selectPaneForProject(state.projects[i]);
        return id;
      }
    }
    return null;
  }

  // 「加入」已成功（"added"）的標記，在最新投影的偵測區不再列著該 repo 時刪掉（repo-projects task 5.1 fix round 1）：
  // 新 Project 出現、repo 離開偵測區，之後若它被移除又回到偵測區，「加入」就恢復可按。"sending" 不在這裡刪——請求還沒有
  // 結果，由 send() 的結果決定。render.js 的 paint() 每次重畫前呼叫；只改狀態、不 repaint()。
  function pruneAddingRepos(state) {
    var detected = state && Array.isArray(state.detected_repos) ? state.detected_repos : [];
    for (var repo in ui.addingRepos) {
      if (!Object.prototype.hasOwnProperty.call(ui.addingRepos, repo) || ui.addingRepos[repo] !== "added") {
        continue;
      }
      var listed = false;
      for (var i = 0; i < detected.length; i += 1) {
        if (detected[i] && detected[i].repo === repo) {
          listed = true;
          break;
        }
      }
      if (!listed) {
        delete ui.addingRepos[repo];
      }
    }
  }

  // 投影中這個 id 的 Project 是不是 Repo Project（`kind === "repo"`；config 與任何未知的新值都不是）。是就回傳它。
  function repoProjectIn(state, id) {
    var projects = state && Array.isArray(state.projects) ? state.projects : [];
    for (var i = 0; i < projects.length; i += 1) {
      if (projects[i] && projects[i].id === id) {
        return projects[i].kind === "repo" ? projects[i] : null;
      }
    }
    return null;
  }

  // 開著選單的 Project 已不在投影中、或不再是 Repo Project 時收起選單（repo-projects task 5.2），之後它回來也不會自己
  // 打開。render.js 的 paint() 每次重畫前呼叫；只改狀態、不 repaint()。
  function pruneProjectMenu(state) {
    if (ui.projectMenu !== null && repoProjectIn(state, ui.projectMenu) === null) {
      ui.projectMenu = null;
    }
  }

  window.cockpitActions = {
    uiSnapshot: uiSnapshot,
    clearSelected: clearSelected,
    setSelectedProject: setSelectedProject,
    applyPendingProjectSelection: applyPendingProjectSelection,
    flushPaneRowScroll: flushPaneRowScroll,
    pruneAddingRepos: pruneAddingRepos,
    pruneProjectMenu: pruneProjectMenu,
    selectPane: selectPane,
  };

  function seg(value) {
    return encodeURIComponent(value);
  }

  // 每次操作（任何按鈕）配一個遞增序號。錯誤訊息只記「最近一次操作」的：慢的舊請求在較新
  // 的操作之後才失敗時，它的錯誤已經過期，不得蓋掉畫面（task 5.3 fix round 1）。
  var latestOp = 0;

  // 送出一個寫入請求。2xx 呼叫 onSuccess（若有，參數是 fetch 的 Response，本體尚未讀取）；非 2xx 或請求失敗時呼叫
  // onFailure（若有；不論 op 是不是最近一次操作；參數是要顯示的錯誤訊息，repo-projects task 5.2 的對話框拿它顯示在
  // 對話框內），且 op 仍是最近一次操作時，把錯誤訊息放進 UI 狀態，直到下一次操作或使用者按「關閉」（spec「畫面操作」）。
  function send(op, method, url, body, onSuccess, onFailure) {
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
            onSuccess(response);
          }
          return undefined;
        }
        return response.text().then(function (text) {
          var reason = null;
          try {
            var parsed = JSON.parse(text);
            if (parsed && typeof parsed.error === "string") {
              // 錯誤本體帶 code／params 時依介面語言翻譯，沒有或字典不認得就是原文 error（design D4）。
              reason = tMsg(parsed, parsed.error);
            }
          } catch (e) {
            reason = null;
          }
          if (reason === null) {
            reason = text === "" ? response.statusText : text;
          }
          fail(t("actions.error.http", { status: response.status, label: label, reason: reason }));
        }, function () {
          // 本體讀不到（連線中途斷掉）：仍算失敗，原因用狀態文字。
          fail(t("actions.error.http", { status: response.status, label: label, reason: response.statusText }));
        });
      },
      function (err) {
        fail(t("actions.error.network", { label: label, reason: err && err.message ? err.message : err }));
      }
    );
    function fail(message) {
      if (onFailure) {
        onFailure(message);
      }
      showError(op, message);
    }
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

  // 「加入」送出的預設 stages（spec cockpit-dashboard「Project 切換」：依介面語言，繁中「規劃、實作、審查、完成」，
  // 英文 Plan、Implement、Review、Complete；repo-projects design D9、task 5.1）。文字在 i18n.js 字典，按下當時才查——
  // 介面語言載入時決定、切換一律重新載入頁面，所以每次查到的都是目前語言。
  function defaultStages() {
    return [
      t("actions.defaultStage.plan"),
      t("actions.defaultStage.implement"),
      t("actions.defaultStage.review"),
      t("actions.defaultStage.complete"),
    ];
  }

  // OpenSpec 階段（spec cockpit-dashboard「Project 切換」；openspec-stage-sync task 5.2）：值是送給後端的字串，順序即下拉的
  // 選項順序（「不對應」固定排第一，值為 null／空字串）。「加入」送出的預設對應與介面語言無關，四站各對應同名階段，
  // 與 defaultStages() 同序。
  var PHASES = ["plan", "implement", "review", "complete"];

  function defaultPhases() {
    return PHASES.slice();
  }

  // 投影 project 的 stage_phases 對齊 stages 的版本：缺欄位、比 stages 短、或值不是上面四個字串的位置一律視為 null（不對應），
  // 不丟例外。開啟時的初值與過期檢查都用它，兩邊才比得起來。
  function phasesOf(project) {
    var stages = Array.isArray(project.stages) ? project.stages : [];
    var raw = Array.isArray(project.stage_phases) ? project.stage_phases : [];
    return stages.map(function (_, i) {
      return PHASES.indexOf(raw[i]) !== -1 ? raw[i] : null;
    });
  }

  // 「加入」成功（201 `{"id": ...}`）後記下待自動選定的 id，再重畫一次：投影若已先含這個 id，這次重畫就會選定
  // （見 applyPendingProjectSelection）。本體讀不到或沒有字串 id 時不做事——請求本身已成功，不顯示錯誤。
  function onRepoProjectAdded(response) {
    response.json().then(
      function (body) {
        if (body && typeof body.id === "string" && body.id !== "") {
          ui.pendingProjectSelection = body.id;
          repaint();
        }
      },
      function () {
        /* 本體不是 JSON：沒有 id 可選，略過 */
      }
    );
  }

  // -------------------------------------------------------------------------
  // Repo Project 的管理選單與對話框（spec cockpit-dashboard「Project 切換」；repo-projects design D6／D9、task 5.2）
  // -------------------------------------------------------------------------
  //
  // 對話框是一個 body 底下、#app 之外的 <dialog>（比照 notify.js 的設定面板，design D8）：整頁重畫只換 #app 的子節點，
  // 對話框的節點、已輸入的內容、焦點與游標位置都不受影響（spec「對話框跨重畫保留」；專案 memory「整頁重畫會丟掉只存在
  // DOM 上的狀態」）。內容另存在 `dlg`（模組狀態）：輸入時同步寫進 `dlg`，新增、刪除、排序這類結構改變才依 `dlg` 重建列。
  // 以 showModal() 開啟：頁面其餘部分在開啟期間 inert；Tab／Shift+Tab 在對話框內循環（對話框的 keydown，見
  // dialogFocusables）；Esc（cancel 事件）與「取消」關閉且不送請求；關閉後焦點回到觸發的「⋯」（依最後輸入方式決定要不要
  // 外框，見 focusEl）。
  //
  // 送出（「儲存」／「移除」）才是「畫面操作」：遞增 latestOp、清錯誤，經 send() 送出 PATCH／DELETE；成功（2xx）關閉對話框，
  // 畫面不自行改投影、等 /ws 推送；失敗時對話框不關、內容保留，原因同時顯示在對話框內與頁面錯誤橫幅（send() 的 showError，
  // 英文介面依 code 翻譯）。進行中「儲存」呈 aria-disabled＋aria-busy（不用 disabled：焦點不能掉），再送直接略過。
  //
  // 送出前只做不依後端細節的基本提示（空白名稱、空白或重複的 stage 名稱、沒有 stage）：長度、字元等其餘規則一律交給後端，
  // 以回應的 code 顯示（專案 memory「前端複算後端規則來比對，後端規則一改就靜默誤報」）。名稱與 stage 名稱原樣送出，去除前後
  // 空白由後端做。stages 本體依對話框列的身分送 `from`：原有的列帶它開啟時的 stage 名稱（逐字），新增的列為 null；順序即
  // 列的順序，被刪除的列不送（spec「編輯 stage」）。每列另帶 `phase`（OpenSpec 階段字串或 null），來自該列的階段下拉
  // （openspec-stage-sync task 5.2）。名稱一律經 textContent／input.value 呈現，不以 HTML 插入。

  var DIALOG_KIND = {
    "project-rename": "rename",
    "project-edit-stages": "stages",
    "project-remove": "remove",
  };

  // 程式焦點一律經 output.js 的共用 helper：最後輸入是滑鼠時不畫焦點外框（專案 memory「滑鼠操作沒聚焦任何元素時，之後的
  // 程式 focus() 會被 Chrome 判成 :focus-visible」）。只載 render.js／actions.js 的精簡 harness 退回原生 focus()。
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

  // #app 裡帶這個 data-action 與 data-project 的第一個元素（逐一比對屬性，不組選擇器字串：id 可能含任何字元）。
  function findProjectControl(action, projectId) {
    var nodes = root.querySelectorAll("[data-action]");
    for (var i = 0; i < nodes.length; i += 1) {
      if (nodes[i].getAttribute("data-action") === action && nodes[i].getAttribute("data-project") === projectId) {
        return nodes[i];
      }
    }
    return null;
  }

  function focusFirstMenuItem(projectId) {
    focusEl(findProjectControl("project-rename", projectId));
  }

  // 選單開著時按 Esc 收起（焦點原本在選單或「⋯」上時回到「⋯」）；點選單與「⋯」以外的地方也收起。對話框開著時不處理
  // （頁面其餘部分 inert，Esc 歸對話框）。掛在 document：焦點不在 #app 內時也收得到。
  document.addEventListener("keydown", function (event) {
    if (event.defaultPrevented || ui.projectMenu === null || dlg !== null) {
      return;
    }
    if (event.key !== "Escape" && event.key !== "Esc") {
      return;
    }
    var projectId = ui.projectMenu;
    var active = document.activeElement;
    var hadFocus =
      !!active &&
      root.contains(active) &&
      active.getAttribute("data-project") === projectId &&
      /^project-/.test(active.getAttribute("data-action") || "");
    event.preventDefault();
    ui.projectMenu = null;
    repaint();
    if (hadFocus) {
      focusEl(findProjectControl("project-menu", projectId));
    }
  });

  // 掛在 document 的冒泡階段，晚於 #app 上的 pointerdown 委派：按到「⋯」或選單項目時委派已經處理（切換、開對話框），
  // 這裡略過；其他位置（含別的按鈕）收起選單。被委派同步重畫換掉的舊節點仍能以 closest() 找到它原本所在的舊選單。
  document.addEventListener("pointerdown", function (event) {
    if (ui.projectMenu === null) {
      return;
    }
    var target = event.target;
    if (
      target instanceof Element &&
      (target.closest(".project-menu") !== null || target.closest('[data-action="project-menu"]') !== null)
    ) {
      return;
    }
    ui.projectMenu = null;
    repaint();
  });

  var dialogEl = document.createElement("dialog");
  dialogEl.className = "project-dialog";
  dialogEl.setAttribute("aria-labelledby", "project-dialog-title");
  document.body.appendChild(dialogEl);

  // 開著的對話框：null，或 { kind, project, projectName, name（改名輸入框的值）,
  // rows（stage 列：{ key, from, value, phase }，phase 為 PHASES 的字串或 null＝不對應）,
  // openedStages／openedPhases（開啟時投影的快照，過期檢查用）,
  // error（對話框內顯示的原因，null 為沒有）, invalid（error 指向的輸入框焦點目標，null 為不標任何輸入框）, sending }。
  // 階段下拉的選擇只以 rows[].phase 為準：改選時同步寫進去，列的結構改變（新增、刪除、排序）重建 DOM 時從這裡讀回
  // （openspec-stage-sync task 5.2；專案 memory「整頁重畫會丟掉只存在 DOM 上的狀態」）。
  var dlg = null;
  var rowSeq = 0;

  // 三種對話框；其他值一律不開（fix round 1：新增種類時不會被歸到 else 而變成移除）。
  var DIALOG_KINDS = { rename: true, stages: true, remove: true };

  function openDialog(kind, projectId) {
    var project = repoProjectIn(
      typeof window.cockpitLatestState === "function" ? window.cockpitLatestState() : null,
      projectId
    );
    if (DIALOG_KINDS[kind] !== true || project === null) {
      return;
    }
    if (dlg !== null) {
      closeDialog(false);
    }
    var stages = Array.isArray(project.stages) ? project.stages : [];
    var phases = phasesOf(project);
    dlg = {
      kind: kind,
      project: projectId,
      projectName: project.name,
      name: project.name,
      rows: stages.map(function (stage, i) {
        rowSeq += 1;
        return { key: rowSeq, from: stage, value: stage, phase: phases[i] };
      }),
      // 開啟時的 stages 與階段對應快照（fix round 1；openspec-stage-sync task 5.2）：送出前與最新投影比對，別處改過就不送
      // （見 staleProblem）。
      openedStages: stages.slice(),
      openedPhases: phases.slice(),
      error: null,
      invalid: null,
      sending: false,
    };
    buildDialog();
    if (!dialogEl.open) {
      dialogEl.showModal();
    }
    if (kind === "rename") {
      var input = dialogEl.querySelector("input.project-dialog-name");
      focusEl(input);
      input.select();
    } else if (kind === "stages") {
      focusDialogPart(dlg.rows.length > 0 ? { row: dlg.rows[0].key, part: "input" } : { op: "add" });
    } else {
      // 移除確認：焦點先放在「取消」，誤按 Enter 不會刪掉。
      focusDialogPart({ op: "cancel" });
    }
  }

  // 關閉對話框；restoreFocus 為 true 時焦點回到觸發的「⋯」（找不到——例如 Project 已從投影消失——就不動）。
  function closeDialog(restoreFocus) {
    if (dlg === null) {
      return;
    }
    var projectId = dlg.project;
    dlg = null;
    if (dialogEl.open) {
      dialogEl.close();
    }
    dialogEl.replaceChildren();
    dialogEl.removeAttribute("data-dialog");
    if (restoreFocus) {
      var trigger = findProjectControl("project-menu", projectId);
      if (trigger !== null) {
        focusEl(trigger);
      } else if (window.cockpitFocusHint && typeof window.cockpitFocusHint.focusProjectFallback === "function") {
        // fix round 1：「⋯」已不在（Project 在對話框開著時消失）時落到選定的 Project 項目，不讓焦點掉回 <body>。
        window.cockpitFocusHint.focusProjectFallback();
      }
    }
  }

  function dialogNode(tag, className, text) {
    var node = document.createElement(tag);
    if (className) {
      node.className = className;
    }
    if (text !== undefined && text !== null) {
      node.textContent = text;
    }
    return node;
  }

  function dialogButton(label, attrName, attrValue, type) {
    var button = dialogNode("button", "action-button", label);
    button.type = type || "button";
    button.setAttribute(attrName, attrValue);
    return button;
  }

  // 依 dlg 重建整個對話框內容（開啟時與 stage 列的結構改變時）。
  function buildDialog() {
    dialogEl.setAttribute("data-dialog", dlg.kind);
    dialogEl.setAttribute("role", dlg.kind === "remove" ? "alertdialog" : "dialog");
    var form = dialogNode("form", "project-dialog-form");
    form.noValidate = true;
    var title;
    if (dlg.kind === "rename") {
      title = t("actions.dialog.rename.title", { name: dlg.projectName });
    } else if (dlg.kind === "stages") {
      title = t("actions.dialog.stages.title", { name: dlg.projectName });
    } else if (dlg.kind === "remove") {
      title = t("actions.dialog.remove.title");
    }
    var titleEl = dialogNode("h2", "project-dialog-title", title);
    titleEl.id = "project-dialog-title";
    form.appendChild(titleEl);

    if (dlg.kind === "rename") {
      var field = dialogNode("label", "project-dialog-field");
      field.appendChild(dialogNode("span", "project-dialog-label", t("actions.dialog.rename.label")));
      var input = dialogNode("input", "project-dialog-input project-dialog-name");
      input.type = "text";
      input.autocomplete = "off";
      input.spellcheck = false;
      input.value = dlg.name;
      field.appendChild(input);
      form.appendChild(field);
    } else if (dlg.kind === "stages") {
      var hint = dialogNode("p", "project-dialog-text", t("actions.dialog.stages.hint"));
      hint.id = "project-dialog-text";
      form.appendChild(hint);
      form.appendChild(buildStageList());
      form.appendChild(dialogButton(t("actions.dialog.stages.add"), "data-stage-op", "add"));
    } else if (dlg.kind === "remove") {
      var text = dialogNode("p", "project-dialog-text", t("actions.dialog.remove.text", { name: dlg.projectName }));
      text.id = "project-dialog-text";
      form.appendChild(text);
    }
    if (dlg.kind === "remove") {
      dialogEl.setAttribute("aria-describedby", "project-dialog-text");
    } else {
      dialogEl.removeAttribute("aria-describedby");
    }

    var error = dialogNode("p", "project-dialog-error");
    error.id = "project-dialog-error";
    error.setAttribute("role", "alert");
    form.appendChild(error);

    var actions = dialogNode("div", "project-dialog-actions");
    actions.appendChild(dialogButton(t("actions.dialog.cancel"), "data-dialog-op", "cancel"));
    actions.appendChild(
      dialogButton(
        dlg.kind === "remove" ? t("actions.dialog.remove.confirm") : t("actions.dialog.save"),
        "data-dialog-op",
        "submit",
        "submit"
      )
    );
    form.appendChild(actions);
    dialogEl.replaceChildren(form);
    syncDialogStatus();
  }

  // 一列的 OpenSpec 階段下拉（spec「編輯 stage」；openspec-stage-sync task 5.2）：選項依序「不對應」＋PHASES，option 的
  // value 是送給後端的字串（不對應為空字串）；顯示文字走 i18n。選中的項目取自 row.phase。原生 <select>：鍵盤（Tab 到達、
  // 方向鍵改選）與讀屏器行為都由瀏覽器提供。
  function buildPhaseSelect(row, n) {
    var select = dialogNode("select", "project-dialog-input stage-phase");
    select.setAttribute("aria-label", t("actions.dialog.stages.phaseLabel", { n: n }));
    // 字典鍵寫成字面值（i18n-check.js ① 以字面值掃描用到的鍵，不接受組字串）。
    var options = [
      ["", t("actions.dialog.phase.none")],
      ["plan", t("actions.dialog.phase.plan")],
      ["implement", t("actions.dialog.phase.implement")],
      ["review", t("actions.dialog.phase.review")],
      ["complete", t("actions.dialog.phase.complete")],
    ];
    for (var k = 0; k < options.length; k += 1) {
      var option = dialogNode("option", "", options[k][1]);
      option.value = options[k][0];
      select.appendChild(option);
    }
    select.value = row.phase === null ? "" : row.phase;
    return select;
  }

  function buildStageList() {
    var list = dialogNode("ol", "stage-list");
    var count = dlg.rows.length;
    for (var i = 0; i < count; i += 1) {
      var row = dlg.rows[i];
      var n = i + 1;
      var item = dialogNode("li", "stage-row");
      item.setAttribute("data-row", String(row.key));
      var input = dialogNode("input", "project-dialog-input stage-name");
      input.type = "text";
      input.autocomplete = "off";
      input.spellcheck = false;
      input.value = row.value;
      input.setAttribute("aria-label", t("actions.dialog.stages.nameLabel", { n: n }));
      item.appendChild(input);
      item.appendChild(buildPhaseSelect(row, n));
      var buttons = dialogNode("div", "stage-row-actions");
      var up = dialogButton(t("actions.dialog.stages.up"), "data-stage-op", "up");
      up.setAttribute("aria-label", t("actions.dialog.stages.upLabel", { n: n }));
      if (i === 0) {
        up.setAttribute("aria-disabled", "true");
      }
      var down = dialogButton(t("actions.dialog.stages.down"), "data-stage-op", "down");
      down.setAttribute("aria-label", t("actions.dialog.stages.downLabel", { n: n }));
      if (i === count - 1) {
        down.setAttribute("aria-disabled", "true");
      }
      var del = dialogButton(t("actions.dialog.stages.delete"), "data-stage-op", "delete");
      del.setAttribute("aria-label", t("actions.dialog.stages.deleteLabel", { n: n }));
      buttons.appendChild(up);
      buttons.appendChild(down);
      buttons.appendChild(del);
      item.appendChild(buttons);
      var note = dialogNode("span", "stage-row-note");
      item.appendChild(note);
      syncRowNote(note, row);
      list.appendChild(item);
    }
    return list;
  }

  // 列下方的小字：新增的列標「新增的 stage」，改過名的原有列標「原為 <原名稱>」，其餘不顯示。
  function syncRowNote(note, row) {
    var text = "";
    if (row.from === null) {
      text = t("actions.dialog.stages.added");
    } else if (row.value.trim() !== row.from) {
      text = t("actions.dialog.stages.from", { from: row.from });
    }
    note.textContent = text;
    note.hidden = text === "";
  }

  // 對話框內的錯誤訊息與「儲存」的忙碌狀態（不重建其他節點，輸入框的焦點與游標不受影響）。
  function syncDialogStatus() {
    var error = dialogEl.querySelector(".project-dialog-error");
    if (error !== null) {
      error.textContent = dlg.error === null ? "" : dlg.error;
      error.hidden = dlg.error === null;
    }
    // task 6.1 F4：出錯的輸入框標 aria-invalid＋aria-describedby（指向上面的錯誤訊息）。每次都先清再標，錯誤清掉
    // （dlg.error 為 null）時屬性跟著移除，不會殘留在已修正的欄位上。
    var inputs = dialogEl.querySelectorAll("input");
    for (var k = 0; k < inputs.length; k += 1) {
      inputs[k].removeAttribute("aria-invalid");
      inputs[k].removeAttribute("aria-describedby");
    }
    if (dlg.error !== null && dlg.invalid !== null) {
      var bad = invalidInputNode(dlg.invalid);
      if (bad !== null) {
        bad.setAttribute("aria-invalid", "true");
        bad.setAttribute("aria-describedby", "project-dialog-error");
      }
    }
    var submit = dialogEl.querySelector('[data-dialog-op="submit"]');
    if (submit !== null) {
      if (dlg.sending) {
        submit.setAttribute("aria-disabled", "true");
        submit.setAttribute("aria-busy", "true");
      } else {
        submit.removeAttribute("aria-disabled");
        submit.removeAttribute("aria-busy");
      }
    }
    // fix round 1：請求已送出就不能「取消」（關掉對話框看起來像取消了，PATCH／DELETE 其實照樣生效）。進行中「取消」停用、
    // Esc 不關閉（見 cancel／close 事件），請求結束後恢復。
    var cancel = dialogEl.querySelector('[data-dialog-op="cancel"]');
    if (cancel !== null) {
      if (dlg.sending) {
        cancel.setAttribute("aria-disabled", "true");
      } else {
        cancel.removeAttribute("aria-disabled");
      }
    }
  }

  function rowIndexByKey(key) {
    for (var i = 0; i < dlg.rows.length; i += 1) {
      if (dlg.rows[i].key === key) {
        return i;
      }
    }
    return -1;
  }

  // 焦點目標：{ row: key, part: "input" | "up" | "down" | "delete" }、{ op: "add" | "cancel" | "submit" } 或 { name: true }。
  function focusDialogPart(spec) {
    var node = null;
    if (spec.name) {
      node = dialogEl.querySelector("input.project-dialog-name");
    } else if (spec.row !== undefined) {
      var rows = dialogEl.querySelectorAll(".stage-row");
      for (var i = 0; i < rows.length; i += 1) {
        if (rows[i].getAttribute("data-row") === String(spec.row)) {
          node = spec.part === "input" ? rows[i].querySelector("input") : rows[i].querySelector('[data-stage-op="' + spec.part + '"]');
          break;
        }
      }
    } else if (spec.op === "add") {
      node = dialogEl.querySelector('[data-stage-op="add"]');
    } else if (spec.op) {
      node = dialogEl.querySelector('[data-dialog-op="' + spec.op + '"]');
    }
    focusEl(node);
  }

  // 錯誤指向的輸入框：焦點目標是改名輸入框或某一列的輸入框時才標（焦點在按鈕上的提示不標任何輸入框）。
  function invalidInputNode(spec) {
    if (spec.name) {
      return dialogEl.querySelector("input.project-dialog-name");
    }
    if (spec.row !== undefined && spec.part === "input") {
      var rows = dialogEl.querySelectorAll(".stage-row");
      for (var i = 0; i < rows.length; i += 1) {
        if (rows[i].getAttribute("data-row") === String(spec.row)) {
          return rows[i].querySelector("input");
        }
      }
    }
    return null;
  }

  function setDialogError(message, focusSpec) {
    dlg.error = message;
    dlg.invalid = focusSpec ? focusSpec : null;
    syncDialogStatus();
    if (focusSpec) {
      focusDialogPart(focusSpec);
    }
  }

  // stage 列的結構操作（上移、下移、刪除、新增）：改 dlg.rows、重建、焦點跟著列走。停用（aria-disabled）的鈕不動作。
  function stageOp(op, button) {
    if (button.getAttribute("aria-disabled") === "true") {
      return;
    }
    var rowEl = button.closest(".stage-row");
    var key = rowEl !== null ? Number(rowEl.getAttribute("data-row")) : null;
    var index = key === null ? -1 : rowIndexByKey(key);
    var focus = null;
    if (op === "add") {
      rowSeq += 1;
      dlg.rows.push({ key: rowSeq, from: null, value: "", phase: null });
      focus = { row: rowSeq, part: "input" };
    } else if (index === -1) {
      return;
    } else if (op === "up" && index > 0) {
      dlg.rows.splice(index - 1, 0, dlg.rows.splice(index, 1)[0]);
      focus = { row: key, part: "up" };
    } else if (op === "down" && index < dlg.rows.length - 1) {
      dlg.rows.splice(index + 1, 0, dlg.rows.splice(index, 1)[0]);
      focus = { row: key, part: "down" };
    } else if (op === "delete") {
      dlg.rows.splice(index, 1);
      // 焦點移到原位置的下一列（沒有就上一列）的「刪除」，都沒有就到「新增 stage」。
      var next = dlg.rows[index] || dlg.rows[index - 1];
      focus = next ? { row: next.key, part: "delete" } : { op: "add" };
    } else {
      return;
    }
    dlg.error = null;
    dlg.invalid = null;
    buildDialog();
    focusDialogPart(focus);
  }

  // 送出前的基本提示（見區塊開頭）；有問題時回傳 { message, focus }，沒有回傳 null。
  function dialogProblem() {
    if (dlg.kind === "rename") {
      return dlg.name.trim() === "" ? { message: t("actions.dialog.invalid.blankName"), focus: { name: true } } : null;
    }
    if (dlg.kind !== "stages") {
      return null;
    }
    if (dlg.rows.length === 0) {
      return { message: t("actions.dialog.invalid.noStages"), focus: { op: "add" } };
    }
    var seen = {};
    for (var i = 0; i < dlg.rows.length; i += 1) {
      var name = dlg.rows[i].value.trim();
      if (name === "") {
        return { message: t("actions.dialog.invalid.blankStage", { n: i + 1 }), focus: { row: dlg.rows[i].key, part: "input" } };
      }
      if (Object.prototype.hasOwnProperty.call(seen, name)) {
        return { message: t("actions.dialog.invalid.duplicateStage", { name: name }), focus: { row: dlg.rows[i].key, part: "input" } };
      }
      seen[name] = true;
    }
    return null;
  }

  // 對話框開著期間 Project 在別處被移除、或 stages／階段對應在別處被改過（fix round 1；openspec-stage-sync task 5.2 加上
  // stage_phases）：以最新投影比對，不同就不送——stages 的 from 是開啟時的名稱，照送會覆寫別處的修改或被拒絕；
  // 階段對應照送會蓋掉別處的改動。
  function staleProblem() {
    var latest = repoProjectIn(
      typeof window.cockpitLatestState === "function" ? window.cockpitLatestState() : null,
      dlg.project
    );
    if (latest === null) {
      return { message: t("actions.dialog.stale.gone"), focus: { op: "cancel" } };
    }
    // fix round 2：投影的 stages 缺欄位或不是陣列時不比對（無從判斷是否在別處變更），交給後端以 code 回應。
    if (
      dlg.kind === "stages" &&
      Array.isArray(latest.stages) &&
      (JSON.stringify(latest.stages) !== JSON.stringify(dlg.openedStages) ||
        JSON.stringify(phasesOf(latest)) !== JSON.stringify(dlg.openedPhases))
    ) {
      return { message: t("actions.dialog.stale.stages"), focus: { op: "cancel" } };
    }
    return null;
  }

  function submitDialog() {
    if (dlg === null || dlg.sending) {
      return;
    }
    var problem = dialogProblem() || staleProblem();
    if (problem !== null) {
      setDialogError(problem.message, problem.focus);
      return;
    }
    var current = dlg;
    var url = "/api/repo-projects/" + seg(current.project);
    var method = "PATCH";
    var body;
    if (current.kind === "rename") {
      body = { name: current.name };
    } else if (current.kind === "stages") {
      body = {
        stages: current.rows.map(function (row) {
          // 每一列都帶 phase（字串或 null）；省略在後端等於 null，所以這裡一律明確送出（spec「編輯 stage」）。
          return { name: row.value, from: row.from, phase: row.phase };
        }),
      };
    } else if (current.kind === "remove") {
      method = "DELETE";
      body = undefined;
    } else {
      return;
    }
    // 「畫面操作」：遞增 latestOp、清掉上一次的錯誤。
    var op = (latestOp += 1);
    ui.error = null;
    current.sending = true;
    current.error = null;
    syncDialogStatus();
    repaint();
    send(
      op,
      method,
      url,
      body,
      function () {
        if (dlg === current) {
          closeDialog(true);
        }
      },
      function (message) {
        if (dlg === current) {
          current.sending = false;
          setDialogError(message, null);
        }
      }
    );
  }

  // 輸入：寫回 dlg（不重建，游標與焦點不動），清掉對話框內的提示。
  dialogEl.addEventListener("input", function (event) {
    var target = event.target;
    if (dlg === null || !(target instanceof HTMLInputElement)) {
      return;
    }
    if (target.classList.contains("project-dialog-name")) {
      dlg.name = target.value;
    } else if (target.classList.contains("stage-name")) {
      var rowEl = target.closest(".stage-row");
      var index = rowEl !== null ? rowIndexByKey(Number(rowEl.getAttribute("data-row"))) : -1;
      if (index === -1) {
        return;
      }
      dlg.rows[index].value = target.value;
      var note = rowEl.querySelector(".stage-row-note");
      if (note !== null) {
        syncRowNote(note, dlg.rows[index]);
      }
    }
    if (dlg.error !== null) {
      dlg.error = null;
      dlg.invalid = null;
      syncDialogStatus();
    }
  });

  // 階段下拉改選（openspec-stage-sync task 5.2；spec「選到已被使用的階段時他列改回不對應」）：寫回 dlg.rows[].phase
  // （這才是狀態來源）。選到的階段若已被他列使用，他列的 phase 改回 null，並就地把那列的 <select> 設回「不對應」——
  // 不重建對話框，焦點留在剛操作的下拉上（鍵盤方向鍵可以接著按）。選回「不對應」不影響他列。
  dialogEl.addEventListener("change", function (event) {
    var target = event.target;
    if (dlg === null || !(target instanceof HTMLSelectElement) || !target.classList.contains("stage-phase")) {
      return;
    }
    var rowEl = target.closest(".stage-row");
    var index = rowEl !== null ? rowIndexByKey(Number(rowEl.getAttribute("data-row"))) : -1;
    if (index === -1) {
      return;
    }
    var phase = PHASES.indexOf(target.value) !== -1 ? target.value : null;
    dlg.rows[index].phase = phase;
    if (phase !== null) {
      var rowEls = dialogEl.querySelectorAll(".stage-row");
      for (var i = 0; i < dlg.rows.length; i += 1) {
        if (i === index || dlg.rows[i].phase !== phase) {
          continue;
        }
        dlg.rows[i].phase = null;
        for (var k = 0; k < rowEls.length; k += 1) {
          if (rowEls[k].getAttribute("data-row") === String(dlg.rows[i].key)) {
            var other = rowEls[k].querySelector("select.stage-phase");
            if (other !== null) {
              other.value = "";
            }
          }
        }
      }
    }
    if (dlg.error !== null) {
      dlg.error = null;
      dlg.invalid = null;
      syncDialogStatus();
    }
  });

  // 「儲存」與輸入框裡的 Enter 都走表單送出。
  dialogEl.addEventListener("submit", function (event) {
    event.preventDefault();
    submitDialog();
  });

  dialogEl.addEventListener("click", function (event) {
    var target = event.target instanceof Element ? event.target.closest("[data-stage-op], [data-dialog-op]") : null;
    if (dlg === null || target === null || !dialogEl.contains(target)) {
      return;
    }
    if (target.hasAttribute("data-stage-op")) {
      stageOp(target.getAttribute("data-stage-op"), target);
    } else if (target.getAttribute("data-dialog-op") === "cancel" && !dlg.sending) {
      closeDialog(true);
    }
  });

  // Esc：取消（不送請求）。自己關閉以便把焦點還原到「⋯」。
  dialogEl.addEventListener("cancel", function (event) {
    event.preventDefault();
    if (dlg !== null && !dlg.sending) {
      closeDialog(true);
    }
  });

  // 瀏覽器在沒有經過 cancel 的情況下直接關閉（例如連按 Esc 時 Chrome 的關閉請求規則）時同樣收尾。
  // 自己呼叫 close() 時 dlg 已先清掉；close 事件是排進佇列的，所以另看 dialogEl.open，免得蓋到緊接著開的下一個對話框。
  dialogEl.addEventListener("close", function () {
    if (dlg === null || dialogEl.open) {
      return;
    }
    if (dlg.sending) {
      // 請求進行中被瀏覽器強制關閉（連按 Esc 不經可取消的 cancel）：重新開啟，結果仍顯示在這個對話框。
      dialogEl.showModal();
      focusDialogPart({ op: "submit" });
      return;
    }
    closeDialog(true);
  });

  // Tab／Shift+Tab 在對話框內循環：modal 讓頁面其餘部分 inert，但最後一個元素再按 Tab 仍可能跑到瀏覽器介面。
  function dialogFocusables() {
    var all = dialogEl.querySelectorAll("input, select, button");
    var out = [];
    for (var i = 0; i < all.length; i += 1) {
      if (!all[i].disabled && all[i].getClientRects().length > 0) {
        out.push(all[i]);
      }
    }
    return out;
  }

  dialogEl.addEventListener("keydown", function (event) {
    if ((event.key === "Escape" || event.key === "Esc") && dlg !== null && dlg.sending) {
      // 請求進行中：Esc 不關閉（fix round 1）。在 keydown 就取消預設動作，瀏覽器不會發出關閉請求——只靠 cancel 事件的
      // preventDefault 不夠，連按 Esc 時 Chrome 的關閉請求規則會跳過可取消的 cancel 直接關閉。
      event.preventDefault();
      return;
    }
    if (event.key !== "Tab" || event.altKey || event.ctrlKey || event.metaKey) {
      return;
    }
    var items = dialogFocusables();
    if (items.length === 0) {
      return;
    }
    var active = document.activeElement;
    var first = items[0];
    var last = items[items.length - 1];
    if (event.shiftKey && (active === first || !dialogEl.contains(active))) {
      event.preventDefault();
      focusEl(last);
    } else if (!event.shiftKey && (active === last || !dialogEl.contains(active))) {
      event.preventDefault();
      focusEl(first);
    }
  });

  function perform(el) {
    var data = el.dataset;
    var action = data.action;

    if (action === "notify-settings") {
      // 通知鈴鐺（spec desktop-notifications「通知設定」；cockpit-dashboard「畫面整頁重畫」：鈴鐺不屬於
      // 「畫面操作」，按下不改變其錯誤訊息與進行中的操作狀態；design D8；desktop-launch-notify
      // task 3.3）：比照 select-project 提早處理——不遞增 latestOp、不清 ui.error、不碰 ui.rebind，
      // 也不重畫（面板是 notify.js 在 #app 之外管理的節點）。
      if (window.cockpitNotify && typeof window.cockpitNotify.togglePanel === "function") {
        window.cockpitNotify.togglePanel();
      }
      return;
    }

    if (action === "toggle-language") {
      // 語言切換按鈕（spec ui-language「語言切換按鈕」；design D3）：與鈴鐺同一套提早處理——不算「畫面操作」
      // （不遞增 latestOp、不清 ui.error），也不重畫：setLang 寫 cockpit.lang 後整頁重新載入。
      // 儲存不可用時按鈕本身已停用（render.js），這裡再擋一次，點了什麼都不做。
      var i18n = window.cockpitI18n;
      if (i18n && i18n.canPersist) {
        i18n.setLang(i18n.lang === "zh" ? "en" : "zh");
      }
      return;
    }

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
      // repo-projects task 5.1 fix round 1（控制端裁決：使用者明確選擇優先）：加入成功、新 Project 的投影還沒到時使用者
      // 自己選了某個 Project，就取消待自動選定，投影到達後不把選取切走。只在這裡清——paint() 的「選定的 Project 已不在
      // 投影中時退回第一個」也會呼叫 setSelectedProject()，那不是使用者的選擇。
      ui.pendingProjectSelection = null;
      // project-select-pane task 1.1（spec「Project 切換」）：同時選定這個 Project 的 pane（等同按「看輸出」）；
      // 改綁模式中與沒有可挑的工作線時不動。以最新投影判斷——純前端選取、不寫入，投影落後也無妨。
      selectPaneForProject(projectInLatestState(data.project));
      repaint();
      return;
    }

    if (action === "select-pane" || action === "select-bound-pane") {
      // 選取不是「畫面操作」（spec cockpit-dashboard「畫面操作」只列 task／workstream 的寫入
      // 按鈕；design D8；R19）：不遞增 latestOp、不清 ui.error。否則按下「推進」之後立刻去點
      // pane 看輸出，latestOp 會被選取動作往前推，那筆寫入稍後才失敗時 showError() 會因為
      // op 不符而忽略——寫入失敗被選取動作悄悄吞掉；已經顯示的錯誤訊息也會被下一次選取清掉，
      // 兩者都違反 spec「畫面操作」「頁面顯示錯誤訊息……直到下一次操作或使用者關閉」。
      choosePane(data.runtime, data.pane);
      repaint();
      return;
    }

    if (action === "project-menu") {
      // Repo Project 的「⋯」（spec「Project 切換」；repo-projects task 5.2）：開關選單不是「畫面操作」，比照 select-project
      // 提早處理（不遞增 latestOp、不清錯誤、不碰改綁模式）。打開時焦點移到第一個選單項目（滑鼠按的不呈現外框，見
      // focusEl）；收起時焦點由 paint() 照被按的「⋯」還原。
      var menuProject = data.project;
      var opening = ui.projectMenu !== menuProject;
      ui.projectMenu = opening ? menuProject : null;
      repaint();
      if (opening) {
        focusFirstMenuItem(menuProject);
      }
      return;
    }

    if (action === "project-rename" || action === "project-edit-stages" || action === "project-remove") {
      // 選單項目：收起選單、開對話框（同樣不是「畫面操作」；送出時才是）。對話框在 #app 之外，見下方 openDialog()。
      ui.projectMenu = null;
      repaint();
      openDialog(DIALOG_KIND[action], data.project);
      return;
    }

    if (action === "add-repo" && Object.prototype.hasOwnProperty.call(ui.addingRepos, data.repo)) {
      // 這個 repo 的「加入」進行中或已成功、投影尚未反映（按鈕呈 aria-disabled）：不送第二筆、不算一次「畫面操作」——
      // 不遞增 latestOp、不清錯誤（比照 select-project 的提早處理；repo-projects task 5.1 fix round 1）。
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
    } else if (action === "add-repo") {
      // 加入 Repo Project（spec「Project 切換」；repo-projects task 5.1）：本體只有 repo（投影 detected_repos 的值原樣
      // 送回）、依介面語言的預設 stages 與同序的預設 phases（與語言無關；openspec-stage-sync task 5.2），不帶 name
      // （用 repo 的預設名稱）。是「畫面操作」：上面已遞增 latestOp、清錯誤，
      // 失敗時 send() 顯示錯誤（英文介面依 code 翻譯），成功後畫面不自行改投影，等 /ws 推送。
      var addingRepo = data.repo;
      ui.addingRepos[addingRepo] = "sending";
      send(
        op,
        "POST",
        "/api/repo-projects",
        { repo: addingRepo, stages: defaultStages(), phases: defaultPhases() },
        function (response) {
          // 成功：保留標記到投影的偵測區不再列著它（pruneAddingRepos），避免真後端合併投影前再按一次被 409 拒絕。
          ui.addingRepos[addingRepo] = "added";
          onRepoProjectAdded(response);
        },
        function () {
          // 失敗：恢復可按（錯誤訊息由 send() 照「畫面操作」顯示）。
          delete ui.addingRepos[addingRepo];
          repaint();
        }
      );
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
