// channel.js 只做一件事：連 /ws、收到訊息就呼叫 window.onState(state)、斷線後以退避重連
// （spec cockpit-dashboard「畫面整頁重畫」；設計文件 §8.3）。換 Tauri 之類的殼時只改這個檔。
//
// 載入順序要求：index.html 把 render.js 放在這個檔案之前，這樣 window.onState／
// window.onChannel 在第一次被呼叫前就已經綁好；這裡仍加一層 typeof 防呆，避免任何順序
// 調整意外把整頁弄壞。

(function () {
  "use strict";

  // 退避序列（毫秒）：1、2、4、8 秒，之後維持 8 秒。
  var BACKOFF_STEPS_MS = [1000, 2000, 4000, 8000];
  var backoffIndex = 0;

  function nextDelayMs() {
    var delay = BACKOFF_STEPS_MS[Math.min(backoffIndex, BACKOFF_STEPS_MS.length - 1)];
    if (backoffIndex < BACKOFF_STEPS_MS.length - 1) {
      backoffIndex += 1;
    }
    return delay;
  }

  function notifyChannel(status) {
    if (typeof window.onChannel === "function") {
      window.onChannel(status);
    }
  }

  function connect() {
    var protocol = location.protocol === "https:" ? "wss://" : "ws://";
    var socket = new WebSocket(protocol + location.host + "/ws");

    // onclose 與 onerror 都可能因同一次失敗而觸發（onerror 之後瀏覽器一定還會補一次
    // onclose）；用這個旗標確保一次連線失敗只回報一次斷線、只排一次重連，不會重複開兩條
    // WebSocket。
    var disconnectHandled = false;

    function handleDisconnect() {
      if (disconnectHandled) {
        return;
      }
      disconnectHandled = true;
      notifyChannel("disconnected");
      setTimeout(connect, nextDelayMs());
    }

    socket.onopen = function () {
      backoffIndex = 0;
      notifyChannel("connected");
    };

    socket.onmessage = function (event) {
      if (typeof window.onState === "function") {
        window.onState(JSON.parse(event.data));
      }
    };

    socket.onclose = handleDisconnect;
    socket.onerror = handleDisconnect;
  }

  connect();
})();
