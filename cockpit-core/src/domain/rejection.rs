//! 進度操作與覆蓋設定的拒絕原因。`Display` 直接就是 HTTP 409 body `{"error": "<原因>"}`
//! 要用的字串（spec `pipeline-domain` 「進度操作」、`runtime-binding` 「畫面覆蓋」、
//! `pipeline-progress` 「綁定覆蓋端點」），沿用 `RuntimeError` 的 `thiserror` 慣例。
//! `apply_op`（2.2）與 `validate_override`（2.3）回傳 `Result<_, Rejection>`，本 task 只定義
//! 型別，不實作那兩個函數。

use thiserror::Error;

/// 進度操作或覆蓋設定被拒絕的原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum Rejection {
    /// 推進被拒絕：目前 Stage 已是 Project `stages` 的最後一個。
    #[error("已是最後一個 Stage")]
    AlreadyLastStage,
    /// 推進或重新標記被拒絕：Task 目前已有標記（`completed` 或 `failed`）。
    #[error("已有標記")]
    AlreadyMarked,
    /// 覆蓋設定被拒絕：指定的 runtime 不是設定檔中已登記的 runtime。
    #[error("runtime 未登記")]
    RuntimeNotRegistered,
    /// 覆蓋設定被拒絕：指定的 runtime 目前不是 `connected`。
    #[error("runtime 未連線")]
    RuntimeNotConnected,
    /// 覆蓋設定被拒絕：狀態庫中沒有指定的 pane。
    #[error("pane 不存在")]
    PaneNotFound,
    /// 覆蓋設定被拒絕：指定的 pane 已 exited。
    #[error("pane 已 exited")]
    PaneExited,
}
