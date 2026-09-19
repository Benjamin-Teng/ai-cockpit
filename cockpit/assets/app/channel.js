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

    // 退避只在「這次連線收到第一則訊息」時才歸零，不在 onopen 就歸零：服務接受連線後立刻
    // 關閉、一則訊息都沒送就等於這次連線沒有真的復原，維持退避遞增（spec「連上即斷不歸零
    // 退避」）。每個 socket 各自一份，重新 connect() 會重新宣告。
    var backoffResetPending = true;

    function handleDisconnect() {
      if (disconnectHandled) {
        return;
      }
      disconnectHandled = true;
      notifyChannel("disconnected");
      setTimeout(connect, nextDelayMs());
    }

    socket.onopen = function () {
      notifyChannel("connected");
    };

    socket.onmessage = function (event) {
      // spec 寫的是「收到第一則訊息」歸零，不限解析成功——壞訊息也是「收到訊息」，一樣代表
      // 這次連線真的復原了，所以歸零要放在 JSON.parse 之前，對合法與不合法訊息一視同仁。
      if (backoffResetPending) {
        backoffIndex = 0;
        backoffResetPending = false;
      }
      var state;
      try {
        state = JSON.parse(event.data);
      } catch (e) {
        // 壞訊息不中斷通道：略過這一則、記警告，等下一則正常訊息繼續重畫（spec「壞訊息不
        // 中斷」）。刻意不呼叫 handleDisconnect／不關閉 socket。
        console.warn("channel.js: 收到無法解析為 JSON 的訊息，已略過", e);
        return;
      }
      if (typeof window.onState === "function") {
        window.onState(state);
      }
    };

    socket.onclose = handleDisconnect;
    socket.onerror = handleDisconnect;
  }

  connect();
})();
