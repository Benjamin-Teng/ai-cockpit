# ADR-0004 後端到畫面每次變動推整張圖

- Status: Accepted
- Date: 2026-09-13

## Context

畫面要即時反映後端的整張狀態圖。狀態規模是幾十個 pane，一份 JSON 幾 KB，且只走本機。
HERDR 事件沒有序號，畫面端若做增量合併，斷線後需要自己補洞。

## Decision

`ProjectedState` 每次變動整份序列化推給所有 WebSocket 客戶端，附遞增 `version`。
內容相等就不遞增、不推；50 ms 內的多次變動合併成一次。畫面端收到就整頁重畫。

## Alternatives rejected

- JSON patch 增量：省流量，但畫面端要保序、要處理 gap、要 resync。狀態很大或走網路時
  才划算，MVP 用不到。

## Consequences

- 畫面端沒有合併邏輯，斷線重連只需等下一份。
- 定期重拿 snapshot 不會造成無謂重畫，因為沒變不推。
- 若未來 pane 數量到數百且變動頻繁，再評估增量。
