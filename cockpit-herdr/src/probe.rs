//! WSL 探測（spec「WSL 探測」；design D8）：在對 `wsl` 型 runtime 建立事件流之前，先確認
//! 目標發行版有沒有在運作中——取 snapshot 時不另外探測（連線存活代表虛擬機在跑）。
//!
//! 解碼策略取自 spike 5（`docs/research/2026-09-13/change-1a-spikes.md`）的實測：
//! `wsl.exe --list --running --quiet` 預設輸出 UTF-16LE（每字元後接一個 `0x00` byte）；
//! 設了 `WSL_UTF8=1` 時輸出變成不含 NUL 的 UTF-8。`String::from_utf8().is_ok()` 對 UTF-16LE
//! 輸出也可能回 `Ok`（NUL byte 本身是合法的單位元組 UTF-8 字元 U+0000，交錯出現只是偽陽性），
//! 所以判別只能看 stdout 有沒有 NUL byte，不能看 `from_utf8` 是否成功。

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use cockpit_core::{Message, RuntimeError};
use tokio::process::Command;

/// Windows `CREATE_NO_WINDOW`：啟動 `wsl.exe` 不彈出主控台視窗（同
/// `herdr-client/src/connector/child_stdio.rs` 的做法）。直接硬寫常數，不為它引入
/// `windows-sys`。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 把 `wsl.exe --list ...` 的原始 stdout bytes 解碼成清單：含 NUL byte 視為 UTF-16LE，否則
/// 視為 UTF-8；之後依 `\n` 切行、裁掉每行的 `\r` 與前後空白、丟掉空行。
pub fn decode_list(bytes: &[u8]) -> Vec<String> {
    let decoded = if bytes.contains(&0u8) {
        if !bytes.len().is_multiple_of(2) {
            tracing::warn!(
                len = bytes.len(),
                "WSL 探測輸出位元組長度為奇數，UTF-16LE 解碼將捨棄最後一個 byte"
            );
        }
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };

    decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// 一次 `wsl.exe --list ...` 執行完的原始結果，尚未判讀——判讀邏輯在 [`evaluate`]。
#[derive(Debug)]
pub struct ProbeOutcome {
    /// 指令本身是否成功結束（`ExitStatus::success()`）；**不代表**目標發行版有沒有在清單裡，
    /// 只代表指令有沒有跑起來。
    pub status_ok: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// 依探測指令的原始結果判斷 `distro` 是否在運作中（spec「WSL 探測」）。
///
/// 判斷順序：
/// 1. 指令無法執行（`Err(io)`）→ `Unavailable`，原因「WSL 探測失敗：{io}」。
/// 2. 指令執行了但非零結束（`status_ok == false`）→ `Unavailable`，原因「WSL 探測失敗：」
///    加上去除頭尾空白的 stderr。
/// 3. 指令成功結束，解碼 stdout 得到的清單含 `distro`（逐行完全相等）→ `Ok(())`。
/// 4. 指令成功結束但清單不含 `distro` → `Unavailable`，原因「WSL 發行版 {distro} 未啟動」。
///
/// **不看 exit code 判斷有沒有發行版**：spike 5 實測顯示有無發行版時 `wsl.exe` 的 exit code
/// 都是 0，`status_ok` 只用來判斷指令本身有沒有失敗。兩種 `Unavailable` 都附上呼叫端傳入的
/// 固定重試間隔（design D4）。
pub fn evaluate(
    distro: &str,
    outcome: Result<ProbeOutcome, std::io::Error>,
    retry_after: Duration,
) -> Result<(), RuntimeError> {
    let outcome = outcome.map_err(|io| RuntimeError::Unavailable {
        reason: Message::WslProbeFailed {
            detail: io.to_string(),
        }
        .text(),
        retry_after,
    })?;

    if !outcome.status_ok {
        let stderr = String::from_utf8_lossy(&outcome.stderr);
        return Err(RuntimeError::Unavailable {
            reason: Message::WslProbeFailed {
                detail: stderr.trim().to_string(),
            }
            .text(),
            retry_after,
        });
    }

    let running = decode_list(&outcome.stdout);
    if running.iter().any(|line| line == distro) {
        Ok(())
    } else {
        Err(RuntimeError::Unavailable {
            reason: Message::WslDistroNotRunning {
                distro: distro.to_string(),
            }
            .text(),
            retry_after,
        })
    }
}

/// 探測某個 WSL 發行版是否在運作中；`Ok(())` 代表在運作中，`Err` 一律是
/// [`RuntimeError::Unavailable`]（探測失敗或發行版未啟動都附固定重試間隔，design D4）。
#[async_trait]
pub trait DistroProber: Send + Sync {
    async fn probe(&self, distro: &str) -> Result<(), RuntimeError>;
}

/// 真實現：跑 `wsl.exe --list --running --quiet`（spec「WSL 探測」；design D8）。
pub struct WslProber {
    /// 探測失敗或發行版未啟動時建議的固定重試間隔（`wsl_probe_secs`，design D4）。
    pub retry_after: Duration,
}

#[async_trait]
impl DistroProber for WslProber {
    async fn probe(&self, distro: &str) -> Result<(), RuntimeError> {
        let mut cmd = Command::new("wsl.exe");
        cmd.args(["--list", "--running", "--quiet"]);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let outcome = cmd.output().await.map(|output| ProbeOutcome {
            status_ok: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        });

        evaluate(distro, outcome, self.retry_after)
    }
}
