// output.js：Live Output 面板與輪詢（spec live-output「輪詢與顯示」「失敗與消失的呈現」；
// design D8）。這個檔案擁有 `<section id="output">`——它自己在載入時把面板的子節點建出來，
// 不靠 index.html 內嵌骨架，也不會被 render.js 的 `replaceChildren` 換掉（那只作用在
// `#app`）。對外只暴露 `window.liveOutput = { select(runtime, paneId), clear(), setKnownPanes(panes),
// tabHidden(change), tabShown(change) }`（後兩者見下方「分頁可見性」）；
// 「誰被選」這個 UI 狀態放在 actions.js（design D8），這裡只記自己輪詢需要的最小狀態。
//
// 輪詢核心（live-output task 5.2 逐字要求；G4 fix wave Finding 1／R20 加 AbortController）：
//   - 沒有選取時不發請求。
//   - `setTimeout` 串接（不用 `setInterval`）：前一次請求「結束」（成功、失敗或逾時）後
//     1 秒才排下一次。
//   - 同一時間至多一個進行中的請求——不只是「觀察 10 秒符合」，是結構上不可能有第二個：
//     每次發請求建立一個 `AbortController`，`inFlightController` 追蹤目前那一個（`null` 表示
//     沒有）；`select()`／`clear()`／`markGone()`／按「取消選取」時都會 `abortInFlight()` 中止它，
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
// 「標為過期」（direction-01-visual task 4.2；design D7）：`#output` 加 `is-stale` class
// （`setStale()`），CSS 依此把內容文字改成 `--text-dim`、面板左緣加一條 `--warn` 色條
// （見 style.css），`setStale()` 同時切換標題列「過期」文字（`.output-stale-label`，
// `staleLabelEl`）的顯示。不再用 opacity（會把文字對比拉到 4.5:1 以下，違反「Direction 01
// 視覺語彙」）。`getComputedStyle` 驗得到看得見的差異，不只是加 class。
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
// 「取消選取」按鈕（原「關閉」）與「pane 已不存在」都要回呼 actions.js 清掉 `ui.selected`（design D8）：兩者
// 都呼叫 `window.cockpitActions.clearSelected()`（若存在）。這個回呼只在 output.js **自己**
// 決定要停止時才觸發；外部呼叫 `window.liveOutput.clear()`（例如 actions.js 自己已經在處理
// `ui.selected` 的改變時）不會再呼叫回去，避免來回互叫。
//
// 分頁可見性（file-review task 4.1；design D6；spec live-output「輪詢與顯示」）：`#output` 現在是中欄
// 下半部分頁區第一個分頁（Live Output）的內容，由 files.js 切換分頁。對 files.js 多兩個入口：
//   - `tabHidden(change)`：分頁變為不可見。先記下內容框是否貼底與捲動位置，再執行 `change()`（呼叫端
//     把 tabpanel 設 hidden）；之後不再發新的輸出請求——已經發出的那一個照常完成（不 abort），但它
//     settle 後排的下一次輪詢在不可見期間一律不發。
//   - `tabShown(change)`：分頁變為可見。執行 `change()`（拿掉 hidden）後寫回切走時記下的捲動狀態——
//     切走前貼底就捲到底、否則回到原本的 scrollTop（被 hidden 的捲動容器 scrollTop 不保證保留）；
//     有選取時立即請求一次（切回時還沒完成的舊請求先 abort 並遞增世代序號淘汰，fix round 1）。之後到達的新內容照 writeText() 的貼底判定跟著走（spec「切回時保持貼底」
//     含切回後立即到達的新內容）。記下／寫回沿用 keepPinnedAcross() 同一組 capturePin()／restorePin()，
//     只是跨越一段時間（memory stick-to-bottom-lost-when-container-resizes）。
// 以任何方式選定 pane（select()）時通知 files.js（`window.cockpitFiles.paneSelected`），由它切到 Live
// Output 分頁；選取被清掉（取消選取、外部 clear()、pane 已不存在）時通知 `paneCleared`，不切換分頁。

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
  // 空狀態文案（direction-01-visual task 4.1；design D7 逐字）：不提位置（760–1199 與 <760 時
  // runtime 清單不在右側），直接用畫面上的按鈕名稱。
  var EMPTY_TEXT = "還沒選 pane。點 runtime 清單裡的任一列，或按 Factory Floor 列首的「看輸出」。";

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
      tabHidden: function (change) {
        change();
      },
      tabShown: function (change) {
        change();
      },
    };
    return;
  }

  var emptyEl, titleEl, staleLabelEl, closeButton, truncatedNotice, goneNotice, reasonEl, preEl;

  // 面板常駐（direction-01-visual task 4.1；design D7）：骨架一載入就可見。沒有選取時只畫出
  // 空狀態（`.output-empty`），標題列、各提示與內容框都收起；有選取（`#output.is-open`）時反過來。
  // 兩種狀態的切換只靠 `.is-open` 這一個 class，由 style.css 決定哪些子節點畫出來
  // （`.output-panel:not(.is-open) > :not(.output-empty)` 與 `.output-panel.is-open > .output-empty`），
  // 各提示節點自己的 `hidden` 仍只表達「這則提示此刻該不該出現」，兩者互不干擾。
  function buildSkeleton() {
    outputSection.classList.add("output-panel");
    outputSection.textContent = "";
    // task 4.1 fix round 1：取消選取後，原本那一列 pane 已不在畫面上時的焦點退路（見
    // restoreFocusAfterDeselect()）；-1 只讓程式可以聚焦，不進 Tab 順序。
    outputSection.tabIndex = -1;

    emptyEl = document.createElement("div");
    emptyEl.className = "output-empty";
    emptyEl.textContent = EMPTY_TEXT;
    outputSection.appendChild(emptyEl);

    var header = document.createElement("div");
    header.className = "output-header";

    // task 4.2（design D7）：標題（runtime／pane id）與「過期」文字放進同一個群組
    // （.output-title-group），兩者緊鄰、跟著標題一起被推到標題列左側；.output-header 本身仍只有
    // 兩個直接子節點（這個群組＋closeButton），justify-content: space-between 的行為不變。
    var titleGroup = document.createElement("div");
    titleGroup.className = "output-title-group";

    titleEl = document.createElement("span");
    titleEl.className = "output-title";
    titleGroup.appendChild(titleEl);

    // 標題列「過期」文字（design D7；跟內容文字 --text-dim、面板左緣 --warn 色條、原因訊息
    // --warn 同一組語彙）：預設隱藏，setStale() 切換。
    staleLabelEl = document.createElement("span");
    staleLabelEl.className = "output-stale-label";
    staleLabelEl.textContent = "過期";
    staleLabelEl.hidden = true;
    titleGroup.appendChild(staleLabelEl);

    header.appendChild(titleGroup);

    closeButton = document.createElement("button");
    closeButton.type = "button";
    closeButton.className = "action-button output-close";
    closeButton.textContent = "取消選取"; // live-output delta spec「取消選取」（原「關閉」）
    closeButton.addEventListener("click", handleDeselectClick);
    header.appendChild(closeButton);

    outputSection.appendChild(header);

    // 失敗原因（503／504／其他非 2xx／請求本身失敗）；文字一律 textContent 寫入，見
    // showReason()。跟 goneNotice 分開一個節點：「pane 已不存在」不需要（也不應該）疊加上一輪
    // 重試留下的原因文字。
    // task 4.2 fix round 1（Codex medium／設計 I1／Ruling R40）：排在截斷提示之前，緊貼標題列——
    // 截斷提示改用 --text-dim 之後是常駐的資訊性提示（見 truncatedNotice 建立處），原因訊息才是
    // 「現在有問題」的訊號，兩者同時出現時原因應該先被看到，不能被截斷提示擠到第三行。
    reasonEl = document.createElement("div");
    reasonEl.className = "output-error-reason";
    reasonEl.hidden = true;
    outputSection.appendChild(reasonEl);

    // 截斷提示（spec live-output「截斷提示」：面板頂端出現「更早的輸出未顯示」）：改版前用
    // --warn，跟過期色條／標籤／原因訊息撞色——task 4.2 design 審核 I1／Ruling R40：真機的
    // `read_output` 固定用 `ReadSource::Recent`，`truncated` 幾乎恆為 true（見
    // docs/research/2026-09-19/pane-read-probe.md §3），這則提示因此近乎常駐，用警示色會稀釋
    // 「過期」語彙。它只是說明「這是最近幾行、不是從頭」的常駐資訊，改用 --text-dim（見
    // style.css），跟「pane 已不存在」（--bad，永久消失）、原因訊息（--warn，暫時性失敗）三種
    // 語意分開。排在 reasonEl 之後、仍在 preEl（輸出內容）之前，滿足 spec「頂端出現」（頂端＝
    // 內容區頂端，在實際輸出文字之上）。
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

    preEl = document.createElement("pre");
    preEl.className = "output-text";
    outputSection.appendChild(preEl);
  }

  buildSkeleton();

  // --- 選取狀態（output.js 自己需要的最小狀態；「誰被選」的 UI 狀態仍在 actions.js）---

  var generation = 0;
  var current = null; // { runtime, paneId } | null；gone 之後仍保留（給標題用），直到 clear()／取消選取／重新 select()
  var gone = false;
  var lastRenderedText = null;
  var pollTimer = null;
  // 目前追蹤的那個進行中請求的 AbortController；null 表示沒有請求在飛（G4 fix wave R20）。
  // 用它（而不是單一布林值）當「這次 settle 是不是我要的那個請求」的身分識別，見 runPoll()。
  var inFlightController = null;
  // Live Output 分頁是否為目前分頁（file-review task 4.1）：false 時 runPoll() 不發新請求。
  var visible = true;
  // 切走時記下的內容框捲動狀態（capturePin() 的結果）；切回時寫回後清成 null。
  var savedPin = null;

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

  // `.is-open`＝「有選取」（design D7 保留這個意義）：標題、提示、內容框與「取消選取」畫出來，
  // 空狀態收起。
  function showPanel() {
    outputSection.classList.add("is-open");
  }

  // 回到空狀態（direction-01-visual task 4.1；spec live-output「取消選取」：「面板回到空狀態」）。
  // 面板本身不收起；上一個選取留下的標題、內容、提示與過期標示一併清掉，不留在 DOM 裡等下次
  // 選取才被蓋掉（收起的節點仍在 textContent 裡，輔助工具與腳本都讀得到）。
  function showEmptyState() {
    outputSection.classList.remove("is-open");
    titleEl.textContent = "";
    preEl.textContent = "";
    lastRenderedText = null;
    truncatedNotice.hidden = true;
    goneNotice.hidden = true;
    hideReason();
    setStale(false);
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

  // 內容框上方的提示行（截斷提示、失敗原因、「pane 已不存在」）出現或消失會改變內容框的高度：
  // 寬 ≥760 時面板高度由版面決定，提示行一出現內容框就變矮，scrollTop 不變的話原本貼底的最後
  // 幾行會被擠出可視範圍、最後一行被切一半，之後也不再被判定為貼底（direction-01-visual task
  // 5.1，4.2 觀察）。切換前原本貼底，切換後就重新捲到底；使用者往上捲（不貼底）時不動，不把人
  // 拉回去（spec live-output「往上捲不被拉回」）。
  // 記下與寫回拆成兩半（file-review task 4.1）：keepPinnedAcross() 在同一個同步區段裡前後呼叫；
  // 分頁切走／切回（tabHidden()／tabShown()）則跨越一段時間，切走時記下、切回時寫回。
  function capturePin() {
    return { pinned: isPinnedToBottom(), top: preEl.scrollTop };
  }

  function restorePin(saved) {
    if (saved.pinned) {
      preEl.scrollTop = preEl.scrollHeight;
    }
  }

  function keepPinnedAcross(change) {
    var saved = capturePin();
    change();
    restorePin(saved);
  }

  function setTruncated(isTruncated) {
    keepPinnedAcross(function () {
      truncatedNotice.hidden = !isTruncated;
    });
  }

  // 「標為過期」（direction-01-visual task 4.2；design D7）：加／拿掉 #output 的 is-stale
  // class，CSS 依此把內容文字改 --text-dim、面板左緣加 --warn 色條（見 style.css），
  // getComputedStyle 驗得到看得見的差異——brief「精確值」明文要求不能只加 class。同步切換
  // 標題列「過期」文字（staleLabelEl）：三者（內容文字、左緣色條、標題文字）跟 is-stale 這一個
  // class 的生滅完全同步，不會有其中一項忘了跟著切換。
  function setStale(isStale) {
    if (isStale) {
      outputSection.classList.add("is-stale");
    } else {
      outputSection.classList.remove("is-stale");
    }
    staleLabelEl.hidden = !isStale;
  }

  function showReason(text) {
    keepPinnedAcross(function () {
      reasonEl.textContent = text;
      reasonEl.hidden = false;
    });
  }

  function hideReason() {
    keepPinnedAcross(function () {
      reasonEl.hidden = true;
      reasonEl.textContent = "";
    });
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

  // file-review task 4.1（design D6）：選取改變時通知 files.js——選定時由它切到 Live Output 分頁
  // （spec live-output「選定一個 pane」：以任何方式選定 pane 時切換；取消選取不切換），檔案樹也
  // 依選取換根目錄（file-review task 4.2）。git-review task 4.2：同時通知 git.js（左欄「變更」
  // 面板的根目錄來源同一個選取），它只在自己的「變更」分頁可見時才會真的發請求（見 git.js
  // 檔頭「變更面板」）。
  function notifyFilesSelected(runtime, paneId) {
    if (window.cockpitFiles && typeof window.cockpitFiles.paneSelected === "function") {
      window.cockpitFiles.paneSelected(runtime, paneId);
    }
    if (window.cockpitGit && typeof window.cockpitGit.paneSelected === "function") {
      window.cockpitGit.paneSelected(runtime, paneId);
    }
  }

  function notifyFilesCleared() {
    if (window.cockpitFiles && typeof window.cockpitFiles.paneCleared === "function") {
      window.cockpitFiles.paneCleared();
    }
    if (window.cockpitGit && typeof window.cockpitGit.paneCleared === "function") {
      window.cockpitGit.paneCleared();
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
  // wave R20）：select()／clear()／markGone()／取消選取都呼叫這個函式，接著都需要能立刻判斷「現在
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
    // !visible：Live Output 不是目前分頁時不發新請求（file-review task 4.1；spec live-output「檔案
    // 分頁期間不請求輸出」）。切回時 tabShown() 會立即重新排一次。
    if (current === null || gone || !visible || inFlightController !== null) {
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
    keepPinnedAcross(function () {
      goneNotice.hidden = false;
    });
    hideReason(); // 「pane 已不存在」是完整訊息，不疊加上一輪 503 可能留下的原因文字。
    setStale(true); // 保留最後一份文字（不清空 <pre>），但標為過期。
    notifyClosedExternally();
    notifyFilesCleared();
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
    // 上一個選取在切走時記下的捲動狀態不適用新選取（內容框剛清空，從貼底開始）。
    savedPin = null;
    // 先通知 files.js 切到 Live Output 分頁（會同步呼叫 tabShown()，visible 變回 true），再排輪詢。
    notifyFilesSelected(runtime, paneId);
    schedulePoll(0);
  }

  function clear() {
    abortInFlight();
    generation += 1;
    current = null;
    gone = false;
    cancelScheduledPoll();
    showEmptyState();
    notifyFilesCleared();
  }

  // Live Output 分頁變為不可見（file-review task 4.1；見檔頭「分頁可見性」）：先記下貼底狀態再執行
  // change()（被 hidden 之後量不到捲動尺寸）；不 abort 已經發出的請求（spec「切走後至多再完成一個
  // 先前已發出的請求」），只取消排定中的下一次。
  function tabHidden(change) {
    if (!visible) {
      change();
      return;
    }
    savedPin = capturePin();
    visible = false;
    cancelScheduledPoll();
    change();
  }

  // Live Output 分頁變為可見：change() 之後寫回捲動狀態，有選取時立即請求一次（spec「切回 Live
  // Output 分頁且有選取時立即請求一次」）。file-review task 4.1 fix round 1（Codex medium）：切走前
  // 發出、到切回時還沒完成的請求（慢或卡住）會讓 runPoll() 因 inFlightController 非 null 直接略過，
  // 新請求要等舊的完成（卡住時等到 6 秒逾時）再加 1 秒——所以切回時比照 select() 先 abortInFlight()
  // 淘汰它、遞增世代序號（之後才到的舊回應與它的逾時回呼都被 onPollSettled 的世代檢查丟棄，不改
  // 面板、不多排輪詢），再立即排新請求。
  function tabShown(change) {
    if (visible) {
      change();
      return;
    }
    visible = true;
    change();
    if (savedPin !== null) {
      if (savedPin.pinned) {
        restorePin(savedPin);
      } else {
        preEl.scrollTop = savedPin.top;
      }
      savedPin = null;
    }
    if (current !== null && !gone) {
      abortInFlight();
      generation += 1;
      schedulePoll(0);
    }
  }

  function handleDeselectClick() {
    if (current === null) {
      return;
    }
    var previous = current;
    abortInFlight();
    generation += 1;
    current = null;
    gone = false;
    cancelScheduledPoll();
    showEmptyState();
    notifyClosedExternally(); // actions.js clearSelected() 會同步整頁重畫
    notifyFilesCleared();
    restoreFocusAfterDeselect(previous);
  }

  // 焦點交接（direction-01-visual task 4.1 fix round 1；Codex medium／設計審核 M3）：「取消選取」
  // 按鈕隨空狀態收起，焦點若不處理就掉到 body、下一次 Tab 回到頁首。改成回到剛取消的那一列
  // pane（重畫之後才找，拿到的是新節點；以 data-runtime／data-pane 逐一比對，不拼選擇器字串，
  // id 裡的特殊字元不必跳脫）；那一列已不在畫面上或此刻不可選（pane 已不存在、改綁模式）時
  // 退回面板本身。preventScroll：不為了還原焦點而捲動頁面或區塊。
  function restoreFocusAfterDeselect(target) {
    var rows = document.querySelectorAll('.pane-row[data-action="select-pane"]');
    for (var i = 0; i < rows.length; i += 1) {
      if (
        rows[i].getAttribute("data-runtime") === target.runtime &&
        rows[i].getAttribute("data-pane") === target.paneId
      ) {
        rows[i].focus({ preventScroll: true });
        if (document.activeElement === rows[i]) {
          return;
        }
      }
    }
    outputSection.focus({ preventScroll: true });
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
    tabHidden: tabHidden,
    tabShown: tabShown,
  };
})();
