// output.js：Live Output 面板與輪詢（spec live-output「輪詢與顯示」「失敗與消失的呈現」；
// design D8）。這個檔案擁有 `<section id="output">`——它自己在載入時把面板的子節點建出來，
// 不靠 index.html 內嵌骨架，也不會被 render.js 的 `replaceChildren` 換掉（那只作用在
// `#app`）。對外只暴露 `window.liveOutput = { select(runtime, paneId), clear(), setKnownPanes(panes) }`；
// 「誰被選」這個 UI 狀態放在 actions.js（design D8），這裡只記自己輪詢需要的最小狀態。
//
// 輪詢核心（live-output task 5.2 逐字要求；G4 fix wave Finding 1／R20 加 AbortController）：
//   - 沒有選取時不發請求。
//   - `setTimeout` 串接（不用 `setInterval`）：前一次請求「結束」（成功、失敗或逾時）後
//     1 秒才排下一次。
//   - 同一時間至多一個進行中的請求——不只是「觀察 10 秒符合」，是結構上不可能有第二個：
//     每次發請求建立一個 `AbortController`，`inFlightController` 追蹤目前那一個（`null` 表示
//     沒有）；`select()`／`clear()`／`markGone()`／關閉面板時都會 `abortInFlight()` 中止它，
//     `select()` 並**立即**對新選取排一次輪詢（不必等舊請求落地）。這個不變量的前提是瀏覽器
//     原生 `fetch` 真的遵守 `AbortSignal`（呼叫 `abort()` 會在網路層結束底層連線）——這是產品
//     實際執行的環境（headless／真實 Chrome），不是憑空假設：`live-output-check.js` N 段用
//     CDP `Network` domain 事件（`requestWillBeSent`／`loadingFailed` 的 `canceled: true`）
//     驗過舊請求真的在網路層被取消、任一時刻進行中的輸出端點請求數不超過一個（G4 fix wave
//     round 2 Finding A／R22）。
//   - abort 造成的 reject 不算失敗：不顯示失敗原因、不標過期、不多排一條輪詢鏈（世代序號
//     檢查仍然保留——abort 不保證底層回應一定不會到，仍可能在別的時序下「成功落地」）。
//   - 前端逾時：單次請求超過 `REQUEST_TIMEOUT_MS`（6 秒，略大於服務端 5 秒逾時）就視同一次
//     失敗（標過期、顯示固定的中文原因、依節奏重試）。逾時判定由 `setTimeout` 自己直接觸發，
//     不透過「abort 之後等 fetch 的 promise reject」——這是防禦性寫法：逾時判定不建立在對
//     fetch 內部行為的假設上，不是因為產品支援忽略 `AbortSignal` 的 `fetch`。`live-output-
//     check.js` O 段額外用一個忽略 `AbortSignal`、永不 settle 的假 `fetch` 驗證這個獨立性，
//     那是模擬「請求卡住」的測試手法，不是宣稱產品支援忽略 abort 的執行環境（G4 fix wave
//     round 2 Finding A／R22：措辭澄清，邏輯未變）。
//   - `inFlightController` 的歸零時機用「這次 settle 是不是我建立的那個 controller」的身分
//     識別（`complete()` 內的 `inFlightController === controller` 比對），不是單一布林值盲目
//     歸零：換選取時舊請求的 reject 回呼可能在新請求已經發出、甚至已經完成之後才執行，若用
//     單一布林值會把新請求的旗標誤清掉，或誤刪目前排定的下一次輪詢（見 `runPoll()`／
//     `abortInFlight()` 註解）。
//   - 世代序號（`generation`）在 select／clear／「pane 已不存在」時遞增；回應回來時序號
//     不符就丟棄，不動面板、不多排輪詢（`select()` 自己已經在换選取當下排好了新的輪詢，不再
//     需要靠舊請求的 settle「補發」）。
//   - 文字一律 `textContent` 寫進 `<pre>`；不用 `innerHTML`（spec「內容不被當成 HTML」）。
//   - `text` 與面板目前內容相同時不重寫（也就不會誤觸發捲動）。
//   - 貼底判定：**寫入新內容之前**量 `scrollHeight - scrollTop - clientHeight`，小於
//     NEAR_BOTTOM_PX 才在寫入之後捲到底（量測在 `<pre>` 本身，它是可捲動的內容框）。
//
// 非 200 的處理集中在 `onPollSettled`（brief：「一個集中處理『一次輪詢結果』的函式」）：
//   - 404、或 `setKnownPanes` 發現目前選取已不在投影中 → `markGone()`：顯示「pane 已不存在」
//     （逐字）、停止輪詢、保留最後一份文字並標為過期（design D8；spec「失敗與消失的呈現」）。
//   - 503／504／其他非 2xx／請求本身失敗（`fetch` reject）→ `markStaleWithReason()`：保留最後
//     一份文字並標為過期、顯示原因（回應本體的 `error` 字串；本體不是 JSON 或沒有 `error` 時
//     退回顯示狀態碼；請求本身失敗時顯示固定的中文說明），依節奏繼續重試（task 5.5）。
//   - 200 → `applySuccess()` 一律先清掉過期標示與原因（即使 `text` 與目前內容相同、
//     `writeText()` 提早 return 不重寫 `<pre>`，也不能連帶略過清除標示——這兩件事分開處理）。
//
// 「標為過期」：`#output` 加 `is-stale` class（`setStale()`），CSS 對 `.output-text` 套用
// `opacity`，`getComputedStyle` 驗得到看得見的差異（brief 要求，不只是加 class）。
// 「顯示原因」：獨立節點 `.output-error-reason`（`reasonEl`），文字一律 `textContent` 寫入
// （spec「輪詢與顯示」「內容不被當成 HTML」的同一個原則，這裡延伸到錯誤原因）。404／
// `markGone()` 的情況不顯示這個節點——「pane 已不存在」本身就是完整的訊息，不疊加上一輪
// 503 可能留下的舊原因文字。
//
// `setKnownPanes(panes)`：`panes` 是 `{ runtime, paneId }` 的陣列，代表目前投影裡還存在的
// 所有 pane（跨 runtime）。若目前選取的 `{runtime, paneId}` 不在其中，視同「pane 已不存在」
// ——跟 404 走同一條 `markGone()`，`markGone()` 內建的 `if (current === null || gone) return;`
// 保證兩條「pane 已不存在」路徑（投影先消失／端點先回 404）中先到的那個生效，另一個晚到時
// 是 no-op：不會重新啟動輪詢、不會遞增世代序號、不會覆蓋已經顯示的訊息（CDP 腳本第四段的
// 「端點回 404」情境額外驗過這個競態：疊加 `COCKPIT_PREVIEW_VANISH_PANE` 讓投影消失晚於
// 404，確認晚到的那個觸發沒有任何可觀察的副作用）。`render.js` 每次重畫後呼叫它。
//
// 「關閉」按鈕與「pane 已不存在」都要回呼 actions.js 清掉 `ui.selected`（design D8）：兩者
// 都呼叫 `window.cockpitActions.clearSelected()`（若存在）。這個回呼只在 output.js **自己**
// 決定要停止時才觸發；外部呼叫 `window.liveOutput.clear()`（例如 actions.js 自己已經在處理
// `ui.selected` 的改變時）不會再呼叫回去，避免來回互叫。

(function () {
  "use strict";

  var POLL_DELAY_MS = 1000;
  // 前端請求逾時（G4 fix wave R20）：略大於服務端 5 秒逾時，單次請求超過這個時間就 abort、
  // 當作一次失敗處理。
  var REQUEST_TIMEOUT_MS = 6000;
  // 「小於數像素」的門檻：freshly 開啟、內容還沒超過一屏時 scrollHeight - scrollTop -
  // clientHeight 為 0，一定小於這個值，預設視為貼底。
  var NEAR_BOTTOM_PX = 6;
  var TRUNCATED_TEXT = "更早的輸出未顯示";
  var GONE_TEXT = "pane 已不存在";
  // 請求本身失敗（fetch reject，例如網路中斷）時顯示的固定中文說明——這種情況沒有回應本體
  // 可以解析出 `error` 欄位（brief「精確值」：fetch reject 也算失敗，原因顯示一段固定的中文
  // 說明）。
  var NETWORK_ERROR_TEXT = "無法連線到伺服器，正在重試";
  // 前端逾時（跟上面的「請求本身失敗」分開一個字串：這是頁面自己放棄等待，不是連線層面的
  // reject，原因不同，顯示的說明也應該不同）。
  var TIMEOUT_REASON_TEXT = "請求逾時（超過 6 秒沒有回應），正在重試";

  function seg(value) {
    return encodeURIComponent(value);
  }

  var outputSection = document.getElementById("output");

  // 沒有 #output（理論上不會，index.html 應該一定有）：整個模組什麼都不做，但仍要暴露一個
  // 不會炸掉呼叫端的空殼，避免其他程式碼呼叫 window.liveOutput.* 時噴例外。
  if (!outputSection) {
    window.liveOutput = {
      select: function () {},
      clear: function () {},
      setKnownPanes: function () {},
    };
    return;
  }

  var titleEl, closeButton, truncatedNotice, goneNotice, reasonEl, preEl;

  function buildSkeleton() {
    outputSection.classList.add("output-panel");
    outputSection.textContent = "";

    var header = document.createElement("div");
    header.className = "output-header";

    titleEl = document.createElement("span");
    titleEl.className = "output-title";
    header.appendChild(titleEl);

    closeButton = document.createElement("button");
    closeButton.type = "button";
    closeButton.className = "action-button output-close";
    closeButton.textContent = "關閉";
    closeButton.addEventListener("click", handleCloseClick);
    header.appendChild(closeButton);

    outputSection.appendChild(header);

    truncatedNotice = document.createElement("div");
    truncatedNotice.className = "output-truncated-notice";
    truncatedNotice.textContent = TRUNCATED_TEXT;
    truncatedNotice.hidden = true;
    outputSection.appendChild(truncatedNotice);

    goneNotice = document.createElement("div");
    goneNotice.className = "output-gone-notice";
    goneNotice.textContent = GONE_TEXT;
    goneNotice.hidden = true;
    outputSection.appendChild(goneNotice);

    // 失敗原因（503／504／其他非 2xx／請求本身失敗）；文字一律 textContent 寫入，見
    // showReason()。跟 goneNotice 分開一個節點：「pane 已不存在」不需要（也不應該）疊加上一輪
    // 重試留下的原因文字。
    reasonEl = document.createElement("div");
    reasonEl.className = "output-error-reason";
    reasonEl.hidden = true;
    outputSection.appendChild(reasonEl);

    preEl = document.createElement("pre");
    preEl.className = "output-text";
    outputSection.appendChild(preEl);
  }

  buildSkeleton();

  // --- 選取狀態（output.js 自己需要的最小狀態；「誰被選」的 UI 狀態仍在 actions.js）---

  var generation = 0;
  var current = null; // { runtime, paneId } | null；gone 之後仍保留（給標題用），直到 clear()／關閉／重新 select()
  var gone = false;
  var lastRenderedText = null;
  var pollTimer = null;
  // 目前追蹤的那個進行中請求的 AbortController；null 表示沒有請求在飛（G4 fix wave R20）。
  // 用它（而不是單一布林值）當「這次 settle 是不是我要的那個請求」的身分識別，見 runPoll()。
  var inFlightController = null;

  function keyOf(runtime, paneId) {
    return runtime + "\u0000" + paneId;
  }

  function buildKeySet(panes) {
    var set = {};
    if (panes) {
      for (var i = 0; i < panes.length; i += 1) {
        set[keyOf(panes[i].runtime, panes[i].paneId)] = true;
      }
    }
    return set;
  }

  // --- 面板顯示 ---

  function showPanel() {
    outputSection.classList.add("is-open");
  }

  function hidePanel() {
    outputSection.classList.remove("is-open");
  }

  function resetPanelForSelection(runtime, paneId) {
    titleEl.textContent = runtime + " / " + paneId;
    preEl.textContent = "";
    lastRenderedText = null;
    truncatedNotice.hidden = true;
    goneNotice.hidden = true;
    hideReason();
    setStale(false);
  }

  function isPinnedToBottom() {
    return preEl.scrollHeight - preEl.scrollTop - preEl.clientHeight < NEAR_BOTTOM_PX;
  }

  function writeText(text) {
    if (text === lastRenderedText) {
      return;
    }
    var pinned = isPinnedToBottom();
    preEl.textContent = text;
    lastRenderedText = text;
    if (pinned) {
      preEl.scrollTop = preEl.scrollHeight;
    }
  }

  function setTruncated(isTruncated) {
    truncatedNotice.hidden = !isTruncated;
  }

  // 「標為過期」：加／拿掉 #output 的 is-stale class，CSS 對 .output-text 套用 opacity（見
  // style.css），getComputedStyle 驗得到看得見的差異——brief「精確值」明文要求不能只加 class。
  function setStale(isStale) {
    if (isStale) {
      outputSection.classList.add("is-stale");
    } else {
      outputSection.classList.remove("is-stale");
    }
  }

  function showReason(text) {
    reasonEl.textContent = text;
    reasonEl.hidden = false;
  }

  function hideReason() {
    reasonEl.hidden = true;
    reasonEl.textContent = "";
  }

  // 成功回應之後統一清除「過期」的兩個視覺線索（標示＋原因）。獨立於 writeText()：即使
  // `text` 與目前內容相同、writeText() 提早 return 不重寫 `<pre>`，過期標示與原因仍然必須
  // 消失（brief「精確值」：「相同內容不重寫 `<pre>`」的最佳化不可以連帶略過清除標示）。
  function clearFailure() {
    setStale(false);
    hideReason();
  }

  // 從非 2xx 回應本體解析失敗原因：本體含合法 JSON 且 `error` 為字串時取用；否則（本體不是
  // JSON、沒有 `error` 欄位）退回顯示狀態碼。請求本身失敗（`fetch` reject，`status` 為
  // `null`、沒有本體可解析）一律顯示固定的中文說明。
  function extractFailureReason(status, bodyText) {
    if (status === null) {
      return NETWORK_ERROR_TEXT;
    }
    if (typeof bodyText === "string" && bodyText.length > 0) {
      try {
        var payload = JSON.parse(bodyText);
        if (payload && typeof payload.error === "string") {
          return payload.error;
        }
      } catch (e) {
        // 本體不是合法 JSON：退回顯示狀態碼。
      }
    }
    return "HTTP " + status;
  }

  function markStaleWithReason(reason) {
    setStale(true);
    showReason(reason);
  }

  function notifyClosedExternally() {
    if (window.cockpitActions && typeof window.cockpitActions.clearSelected === "function") {
      window.cockpitActions.clearSelected();
    }
  }

  // --- 輪詢排程 ---

  function cancelScheduledPoll() {
    if (pollTimer !== null) {
      clearTimeout(pollTimer);
      pollTimer = null;
    }
  }

  function schedulePoll(delayMs) {
    cancelScheduledPoll();
    pollTimer = setTimeout(runPoll, delayMs);
  }

  // 中止目前追蹤的那個進行中請求（若有），並**同步**把 `inFlightController` 清成 null（G4 fix
  // wave R20）：select()／clear()／markGone()／關閉都呼叫這個函式，接著都需要能立刻判斷「現在
  // 沒有請求在飛」（select() 需要立刻排下一次輪詢）——不能等異步的 reject 回呼才清旗標，那樣會
  // 跟「立即對新選取發請求」互相卡住。先把 controller 存到區域變數再呼叫 `abort()`：`abort()`
  // 本身不會同步觸發 reject 回呼（那是之後的 microtask），所以這裡的同步歸零與稍後那個回呼裡
  // 用 `inFlightController === controller` 做的身分比對不會互相干擾。
  function abortInFlight() {
    if (inFlightController !== null) {
      var controller = inFlightController;
      inFlightController = null;
      controller.abort();
    }
  }

  function runPoll() {
    pollTimer = null;
    if (current === null || gone || inFlightController !== null) {
      return;
    }
    var gen = generation;
    var target = current;
    var controller = new AbortController();
    inFlightController = controller;
    var settled = false; // 這次請求是否已經收尾過一次（fetch 真的 settle、或前端逾時）。

    // 收尾（不論觸發來源是 fetch 的 promise 真的 settle、還是下面的前端逾時計時器）：只執行
    // 一次；只有這次收尾仍然是目前追蹤的那個請求時才清掉 `inFlightController`——reject 回呼
    // 可能在新的請求已經發出、甚至已經完成之後才執行，用 `inFlightController === controller`
    // 這種 per-request 的身分識別，不會誤清新請求的旗標（不能用單一布林值盲目歸零）。
    function complete(cb) {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeoutId);
      if (inFlightController === controller) {
        inFlightController = null;
      }
      cb();
    }

    var timeoutId = setTimeout(function () {
      // 前端逾時：不依賴底層 fetch 是否真的因為 abort() 而 reject 才觸發——這是防禦性寫法
      // （逾時判定不建立在對 fetch 內部行為的假設上），不是因為產品支援忽略 AbortSignal 的
      // fetch；測試腳本用忽略 AbortSignal、永不 settle 的假 fetch 驗證這個獨立性，那是模擬
      // 「請求卡住」的測試手法，不是受支援的執行環境（見 live-output-check.js O 段；G4 fix
      // wave round 2 Finding A／R22：措辭澄清，邏輯未變）。逾時本身就是終止這次請求的權威
      // 判定，直接收尾、不等 fetch 的 promise 結算。
      controller.abort();
      complete(function () {
        onPollSettled(gen, false, null, null, TIMEOUT_REASON_TEXT);
      });
    }, REQUEST_TIMEOUT_MS);

    var url = "/api/runtimes/" + seg(target.runtime) + "/panes/" + seg(target.paneId) + "/output";
    fetch(url, { signal: controller.signal })
      .then(function (response) {
        return response.text().then(function (bodyText) {
          return { ok: response.ok, status: response.status, bodyText: bodyText };
        });
      })
      .then(
        function (result) {
          complete(function () {
            onPollSettled(gen, result.ok, result.status, result.bodyText);
          });
        },
        function () {
          complete(function () {
            if (controller.signal.aborted) {
              // 被 select()/clear()/markGone() 主動 abort（前端逾時的 abort 已經在上面的
              // setTimeout 分支處理掉、settled 這時已經是 true，不會走到這裡）：不算失敗，
              // 不顯示原因、不標過期、不多排輪詢鏈——select() 自己已經在换選取當下立即排好了
              // 新的輪詢；clear()/markGone() 則是刻意要停止輪詢。
              return;
            }
            // 請求本身失敗（連線中斷……）：狀態碼未知，走非 2xx 分支。
            onPollSettled(gen, false, null, null);
          });
        }
      );
  }

  // 一次輪詢結果的集中處理點（brief：5.5 加過期標示／原因顯示時，只需要在下面的分支裡加
  // 東西，不必動輪詢排程本身）。`explicitReason` 給前端逾時用（G4 fix wave R20）：略過
  // `extractFailureReason()`，直接用固定的逾時說明。
  function onPollSettled(gen, ok, status, bodyText, explicitReason) {
    if (gen !== generation) {
      // 已經被更新的 select／clear／markGone 取代（含 abort 之後仍然到達的回應）：這個回應
      // 作廢，不得改變面板、不得多排一條輪詢鏈——select() 自己已經在换選取當下排好了新的輪詢。
      return;
    }

    if (ok) {
      applySuccess(bodyText);
      schedulePoll(POLL_DELAY_MS);
      return;
    }

    if (status === 404) {
      markGone();
      return;
    }

    // 503／504／其他非 2xx、請求本身失敗（status 為 null）、或前端逾時（explicitReason 有值）：
    // 保留最後一份文字並標為過期、顯示原因，依節奏繼續重試（spec「失敗與消失的呈現」）。
    var reason = typeof explicitReason === "string" ? explicitReason : extractFailureReason(status, bodyText);
    markStaleWithReason(reason);
    schedulePoll(POLL_DELAY_MS);
  }

  function applySuccess(bodyText) {
    var payload;
    try {
      payload = JSON.parse(bodyText);
    } catch (e) {
      // 不應該發生（端點保證回應是合法 JSON）；防禦性地當成暫時性失敗處理：不覆寫內容，
      // 呼叫端仍會依節奏排下一次。過期標示／原因是否存在維持原樣，不在這裡動它。
      return;
    }
    clearFailure(); // 恢復成功：過期標示與原因消失（即使下面 writeText 判定內容相同而不
    // 重寫 <pre>，這兩件事也要發生——見 clearFailure() 上方註解）。
    writeText(typeof payload.text === "string" ? payload.text : "");
    setTruncated(payload.truncated === true);
  }

  function markGone() {
    if (current === null || gone) {
      return;
    }
    abortInFlight(); // pane 已不存在：中止還在飛的請求（G4 fix wave R20），不必等它落地。
    generation += 1; // 讓 abort 之後仍可能到達的舊回應被丟棄（世代序號檢查兜底，見 runPoll()）。
    cancelScheduledPoll();
    gone = true;
    goneNotice.hidden = false;
    hideReason(); // 「pane 已不存在」是完整訊息，不疊加上一輪 503 可能留下的原因文字。
    setStale(true); // 保留最後一份文字（不清空 <pre>），但標為過期。
    notifyClosedExternally();
  }

  // --- 對外 API ---

  function select(runtime, paneId) {
    // 中止舊選取還在飛的請求（若有）——G4 fix wave R20：abort 之後立即對新選取排一次輪詢，
    // 不等舊請求落地才由 onPollSettled 補發（那會讓切換 pane 被慢／永久 pending 的舊 pane
    // 擋住，違反「pane 內容改變後 3 秒內反映」）。abort 造成的 reject 不算失敗，見 runPoll()。
    abortInFlight();
    generation += 1;
    current = { runtime: runtime, paneId: paneId };
    gone = false;
    resetPanelForSelection(runtime, paneId);
    showPanel();
    schedulePoll(0);
  }

  function clear() {
    abortInFlight();
    generation += 1;
    current = null;
    gone = false;
    cancelScheduledPoll();
    hidePanel();
  }

  function handleCloseClick() {
    if (current === null) {
      return;
    }
    abortInFlight();
    generation += 1;
    current = null;
    gone = false;
    cancelScheduledPoll();
    hidePanel();
    notifyClosedExternally();
  }

  function setKnownPanes(panes) {
    if (current === null || gone) {
      return;
    }
    var known = buildKeySet(panes);
    if (!Object.prototype.hasOwnProperty.call(known, keyOf(current.runtime, current.paneId))) {
      markGone();
    }
  }

  window.liveOutput = {
    select: select,
    clear: clear,
    setKnownPanes: setKnownPanes,
  };
})();
