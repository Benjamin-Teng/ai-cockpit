//! `events.subscribe` 通過探測後要重播的腳本（design D7；spec `herdr-event-subscription`）。

use std::time::Duration;

/// 一條 `events.subscribe` 連線在送出 `subscription_started` 之後依序執行的步驟。
///
/// 每條通過探測（見 `FakeHerdrConfig::with_failing_probe_pane_ids`）的連線依訂閱內容選一條
/// `SubscribeRule` 的腳本從頭執行（fix round 1 finding 4，見
/// `FakeHerdrConfig::with_subscribe_rule`）；腳本跑完（沒有 `Close`／`Abort`／`Hold`）時視同
/// `Close`，正常關閉連線。
#[derive(Debug, Clone)]
pub enum Step {
    /// 推送一行合法事件（`{"event":...,"data":...}`）。
    Event(String),
    /// 推送一行壞資料（非 JSON、或缺 `event`／`data` 欄位），驗證呼叫端「跳過壞行、不中斷串流」
    /// 的行為（spec「壞行與結束」）。
    Malformed(String),
    /// 暫停一段時間再繼續下一步，不佔用額外連線。
    Delay(Duration),
    /// 正常關閉連線（乾淨 EOF）。
    Close,
    /// 非正常中斷連線。**平台行為不同**（fix round 1 finding 1，已查證見
    /// `connection.rs` 的 `BestEffortAbort` 文件註解）：Windows 上用
    /// `NamedPipeServer::disconnect()`，讓對端下一次讀取拿到 I/O 錯誤（區別於 `Close` 的乾淨
    /// EOF）；unix 上沒有能做到「非乾淨中斷」的作法（AF_UNIX 沒有 RST 語意、tokio 的
    /// `UnixStream` 也沒公開 `SO_LINGER`），降級為與 `Close` 相同的乾淨 EOF——這是平台限制，
    /// 不是本 crate 的疏漏。
    Abort,
    /// 保持連線開著，直到對端關閉連線、或整個 `FakeHerdr` 被 drop 為止（後者靠
    /// `abort_all()` 取消 handler）。對端關閉時 handler 會結束，這條連線在
    /// `FakeHerdr::closed_connections` 變成 `true`。
    Hold,
}
