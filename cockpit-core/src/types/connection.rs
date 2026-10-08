//! 記錄與某個 runtime 之間連線本身的狀態（與 `RuntimeSnapshot`／`RuntimeEvent` 描述的
//! agent 狀態是兩回事）。

use std::time::{Duration, SystemTime};

/// 與一個 runtime 之間的連線狀態。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// 正在嘗試建立連線，尚未成功過。
    Connecting,
    /// 連線中。
    Connected {
        /// 這次連線建立的時間。
        since: SystemTime,
        /// 對方回報的 server 版本字串。
        server_version: String,
        /// 這次連線使用的協定版本號。
        protocol: u32,
        /// 最近一次成功取得快照的時間。
        last_snapshot_at: SystemTime,
        /// 這輪連線的沉降重拿（spec `runtime-driver`「連線後沉降重拿」）是否已換上：初次 snapshot 為 `false`；
        /// 沉降重拿觸發之後完成的第一份 snapshot（沉降重拿本身，或它併入的進行中重拿）換上時起為 `true`，
        /// 直到這輪連線結束。剛連上時 HERDR 的狀態可能還沒穩定，不可逆的判定（例如清除消失 pane 的
        /// Repo Project 進度，repo-projects task 4.6）只採信 `true` 之後的 pane 樹。
        settled: bool,
        /// 協定相容性警告；`None` 表示沒有警告。字串必須由 [`crate::Message`] 的 `text()` 產生
        /// （見 `RuntimeError` 的說明），投影才能反推代碼給英文介面翻譯。
        protocol_warning: Option<String>,
    },
    /// 已斷線，附上原因與下次重試前的等待時間。
    Disconnected {
        /// 斷線原因，供顯示與記錄。字串必須由 [`crate::Message`] 的 `text()` 產生
        /// （見 `RuntimeError` 的說明），投影才能反推代碼給英文介面翻譯。
        reason: String,
        /// 距離下次重試還要等多久。
        retry_in: Duration,
    },
}
