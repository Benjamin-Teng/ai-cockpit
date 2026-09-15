//! Runtime 組裝（design D16）：把 [`Config`] 裡的每筆 `[[runtime]]` 交給 `cockpit-herdr`
//! 的工廠（2.7 `cockpit_herdr::build`），組成畫面／狀態庫要用的 [`RuntimeEntry`] 清單。
//!
//! `cockpit` 完全不依賴 `herdr-client`（ADR-0003）：這裡只碰 `cockpit-herdr` 公開的
//! [`cockpit_herdr::HerdrEndpoint`]／[`cockpit_herdr::BuildOptions`]／
//! [`cockpit_herdr::BuiltRuntime`]／[`cockpit_herdr::BuildError`]，具體的 connector
//! 型別（`NamedPipeConnector` 等）不會出現在這個 crate 裡。

use std::sync::Arc;
use std::time::Duration;

use cockpit_core::{AgentRuntime, RuntimeId};

use crate::config::Config;

/// 一筆組裝完成的 runtime：狀態庫要用的 id／kind，加上工廠回的端點描述與可用的 runtime。
pub struct RuntimeEntry {
    /// 這個 runtime 在狀態庫裡的 id，來自設定檔的 `[[runtime]].id`。
    pub id: RuntimeId,
    /// runtime 種類；原樣帶過設定檔的 `[[runtime]].kind`（目前恆為 `"herdr"`）。
    pub kind: String,
    /// 端點描述，取自 `cockpit_herdr::build` 回傳的 `BuiltRuntime::endpoint`
    /// （`Connector::describe()`），供畫面與日誌顯示。
    pub endpoint: String,
    /// 這個 runtime 的 WSL 探測失敗重試間隔；`None` 代表不是 `wsl` 型端點。取自
    /// `HerdrRuntime::wsl_retry_after()`（在轉成 `Arc<dyn AgentRuntime>` 之前讀出，
    /// trait object 上沒有這個方法）。供組裝層與測試觀察 `wsl_probe_secs` 是否確實
    /// 轉傳到工廠（review round 1 finding：原本沒有可偵測回歸的測試）。
    pub wsl_retry_after: Option<Duration>,
    /// 建好、可供驅動器使用的 runtime。
    pub runtime: Arc<dyn AgentRuntime>,
}

/// [`build`] 失敗的原因：對應設定檔裡出錯的那一筆 `[[runtime]]`。
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// 某一筆 `[[runtime]]` 交給 `cockpit-herdr` 的工廠時失敗；附上是哪一筆的 id。
    #[error("runtime {id}：{source}")]
    Factory {
        /// 出錯的那一筆 runtime id。
        id: RuntimeId,
        /// 底層工廠回傳的原因。
        #[source]
        source: cockpit_herdr::BuildError,
    },
}

/// 依 `config.runtimes` 的順序，把每一筆交給 `cockpit-herdr` 的工廠組出 [`RuntimeEntry`]。
///
/// `options.wsl_probe_secs` 取自 `config.polling.wsl_probe_secs`（design D16）；`kind`
/// 原樣帶過設定檔的值。
///
/// # Errors
///
/// 任何一筆交給工廠失敗（目前只有 `Command` 端點的 `argv` 為空，或平台不支援 socket
/// 端點）都會讓整個 `build` 回傳 [`BuildError::Factory`]，附上出錯的那一筆 runtime id。
pub fn build(config: &Config) -> Result<Vec<RuntimeEntry>, BuildError> {
    config
        .runtimes
        .iter()
        .map(|runtime_config| {
            let id = RuntimeId::new(runtime_config.id.clone());
            let options = cockpit_herdr::BuildOptions {
                id: id.clone(),
                wsl_probe_secs: config.polling.wsl_probe_secs,
            };
            let built = cockpit_herdr::build(runtime_config.endpoint.clone(), options).map_err(
                |source| BuildError::Factory {
                    id: id.clone(),
                    source,
                },
            )?;
            let wsl_retry_after = built.runtime.wsl_retry_after();
            Ok(RuntimeEntry {
                id,
                kind: runtime_config.kind.clone(),
                endpoint: built.endpoint,
                wsl_retry_after,
                runtime: built.runtime,
            })
        })
        .collect()
}
