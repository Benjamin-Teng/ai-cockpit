//! 測試共用的假探測器（`FakeProber`），供 task 2.3 起、以及後續 task 2.5／2.6 沿用。
//!
//! 不是獨立的 test binary：檔案放在 `tests/common/`（子目錄）而非 `tests/` 直屬，
//! cargo 不會把它當成獨立的 integration test crate，要靠各個 `tests/*.rs` 用
//! `mod common;` 引入。目前只有 `tests/probe.rs` 引入，部分項目在本 task 尚未用到，
//! 用 `#[allow(dead_code)]` 靜音。

#![allow(dead_code)]

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use cockpit_core::RuntimeError;
use cockpit_herdr::probe::DistroProber;

/// 依序回傳固定結果的假 `DistroProber`；最後一筆結果會在序列用完後持續重複，並記錄每次
/// `probe()` 收到的 `distro` 引數供測試斷言呼叫次數與引數。
pub struct FakeProber {
    responses: Mutex<Vec<Result<(), RuntimeError>>>,
    calls: Mutex<Vec<String>>,
}

impl FakeProber {
    /// 每次 `probe()` 都回 `Ok(())`。
    pub fn ok() -> Self {
        Self::sequence(vec![Ok(())])
    }

    /// 每次 `probe()` 都回同一筆 `Unavailable`。
    pub fn unavailable(reason: impl Into<String>, retry_after: Duration) -> Self {
        Self::sequence(vec![Err(RuntimeError::Unavailable {
            reason: reason.into(),
            retry_after,
        })])
    }

    /// 依序回傳 `responses`；序列用完後最後一筆結果持續重複。`responses` 不可為空。
    pub fn sequence(responses: Vec<Result<(), RuntimeError>>) -> Self {
        assert!(
            !responses.is_empty(),
            "FakeProber::sequence 的序列不能是空的"
        );
        Self {
            responses: Mutex::new(responses),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// 依呼叫順序回傳每次 `probe()` 收到的 `distro` 引數。
    pub fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .expect("FakeProber.calls mutex poisoned")
            .clone()
    }
}

/// `RuntimeError` 沒有 derive `Clone`（`thiserror::Error` 不自動提供），這裡手動複製兩個變體
/// 各自的欄位，供序列最後一筆結果重複回傳用。
fn clone_response(response: &Result<(), RuntimeError>) -> Result<(), RuntimeError> {
    match response {
        Ok(()) => Ok(()),
        Err(RuntimeError::Unavailable {
            reason,
            retry_after,
        }) => Err(RuntimeError::Unavailable {
            reason: reason.clone(),
            retry_after: *retry_after,
        }),
        Err(RuntimeError::Failed(message)) => Err(RuntimeError::Failed(message.clone())),
        Err(RuntimeError::PaneNotFound { pane_id }) => Err(RuntimeError::PaneNotFound {
            pane_id: pane_id.clone(),
        }),
    }
}

#[async_trait]
impl DistroProber for FakeProber {
    async fn probe(&self, distro: &str) -> Result<(), RuntimeError> {
        self.calls
            .lock()
            .expect("FakeProber.calls mutex poisoned")
            .push(distro.to_string());

        let mut responses = self
            .responses
            .lock()
            .expect("FakeProber.responses mutex poisoned");
        if responses.len() > 1 {
            responses.remove(0)
        } else {
            clone_response(&responses[0])
        }
    }
}
