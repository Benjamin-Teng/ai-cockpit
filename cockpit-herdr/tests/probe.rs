//! Task 2.3 驗收測試：WSL 探測（spec「WSL 探測」；design D8）。

mod common;

use std::time::Duration;

use cockpit_core::{Message, RuntimeError};
use cockpit_herdr::probe::{DistroProber, ProbeOutcome, WslProber, decode_list, evaluate};

const RETRY_AFTER: Duration = Duration::from_secs(60);

/// 用 `encode_utf16` 自己產生 UTF-16LE bytes，不依賴任何外部固定值。
fn utf16le_bytes(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn assert_unavailable_reason_eq(err: &RuntimeError, expected: &str) {
    match err {
        RuntimeError::Unavailable { reason, .. } => assert_eq!(reason, expected),
        other => panic!("預期 RuntimeError::Unavailable，實得 {other:?}"),
    }
}

fn assert_unavailable_reason_starts_with_and_contains(
    err: &RuntimeError,
    prefix: &str,
    contains: &str,
) {
    match err {
        RuntimeError::Unavailable { reason, .. } => {
            assert!(
                reason.starts_with(prefix),
                "reason 應以 {prefix:?} 開頭，實得 {reason:?}"
            );
            assert!(
                reason.contains(contains),
                "reason 應含 {contains:?}，實得 {reason:?}"
            );
        }
        other => panic!("預期 RuntimeError::Unavailable，實得 {other:?}"),
    }
}

#[test]
fn decode_utf16le_list() {
    let one_line = utf16le_bytes("Ubuntu-24.04\r\n");
    assert_eq!(decode_list(&one_line), vec!["Ubuntu-24.04".to_string()]);

    let two_lines = utf16le_bytes("Ubuntu-24.04\r\ndocker-desktop\r\n");
    assert_eq!(
        decode_list(&two_lines),
        vec!["Ubuntu-24.04".to_string(), "docker-desktop".to_string()]
    );
}

#[test]
fn decode_utf8_list() {
    let one_line = b"Ubuntu-24.04\r\n".to_vec();
    assert_eq!(decode_list(&one_line), vec!["Ubuntu-24.04".to_string()]);

    // 含空行與純 \r\n 行要被丟掉。
    let with_blank_line = b"Ubuntu-24.04\r\n\r\ndocker-desktop\r\n".to_vec();
    assert_eq!(
        decode_list(&with_blank_line),
        vec!["Ubuntu-24.04".to_string(), "docker-desktop".to_string()]
    );
}

#[test]
fn distro_missing_is_unavailable_with_fixed_interval() {
    let empty_list = ProbeOutcome {
        status_ok: true,
        stdout: b"".to_vec(),
        stderr: b"".to_vec(),
    };
    let err =
        evaluate("Ubuntu-24.04", Ok(empty_list), RETRY_AFTER).expect_err("空清單應回 Unavailable");
    assert_unavailable_reason_eq(&err, "WSL 發行版 Ubuntu-24.04 未啟動");
    assert_eq!(err.retry_after(), Some(RETRY_AFTER));

    let other_distro_only = ProbeOutcome {
        status_ok: true,
        stdout: b"docker-desktop\r\n".to_vec(),
        stderr: b"".to_vec(),
    };
    let err = evaluate("Ubuntu-24.04", Ok(other_distro_only), RETRY_AFTER)
        .expect_err("清單沒有目標發行版應回 Unavailable");
    assert_unavailable_reason_eq(&err, "WSL 發行版 Ubuntu-24.04 未啟動");
    assert_eq!(err.retry_after(), Some(RETRY_AFTER));

    let target_present = ProbeOutcome {
        status_ok: true,
        stdout: b"Ubuntu-24.04\r\n".to_vec(),
        stderr: b"".to_vec(),
    };
    assert!(evaluate("Ubuntu-24.04", Ok(target_present), RETRY_AFTER).is_ok());
}

#[test]
fn command_failure_is_unavailable_with_stderr() {
    let nonzero_exit = ProbeOutcome {
        status_ok: false,
        stdout: b"".to_vec(),
        stderr: b"boom".to_vec(),
    };
    let err = evaluate("Ubuntu-24.04", Ok(nonzero_exit), RETRY_AFTER)
        .expect_err("指令失敗應回 Unavailable");
    assert_unavailable_reason_starts_with_and_contains(&err, "WSL 探測失敗：", "boom");
    assert_eq!(err.retry_after(), Some(RETRY_AFTER));

    let cannot_execute = std::io::Error::new(std::io::ErrorKind::NotFound, "no wsl");
    let err = evaluate("Ubuntu-24.04", Err(cannot_execute), RETRY_AFTER)
        .expect_err("指令無法執行應回 Unavailable");
    assert_unavailable_reason_starts_with_and_contains(&err, "WSL 探測失敗：", "no wsl");
    assert_eq!(err.retry_after(), Some(RETRY_AFTER));
}

/// 本機有 WSL 且已啟動 `Ubuntu-24.04` 時手動執行：
/// `cargo test -p cockpit-herdr --test probe -- --ignored`。
/// 虛擬機可能沒在跑，`Ok(())` 與「未啟動」的 `Unavailable` 都算通過；只有真正的
/// 「WSL 探測失敗」（指令跑不起來或非零結束）才視為失敗。
#[tokio::test]
#[ignore = "需要本機安裝 WSL；手動執行 cargo test -p cockpit-herdr --test probe -- --ignored"]
async fn real_wsl_probe_lists_running_distro() {
    let prober = WslProber {
        retry_after: RETRY_AFTER,
    };
    let result = prober.probe("Ubuntu-24.04").await;
    match &result {
        Ok(()) => {
            println!("real_wsl_probe_lists_running_distro: Ok(()) — Ubuntu-24.04 在運作中");
        }
        Err(RuntimeError::Unavailable { reason, .. }) if reason.starts_with("WSL 發行版") => {
            println!("real_wsl_probe_lists_running_distro: {reason}（虛擬機可能沒在跑，視為通過）");
        }
        Err(other) => panic!("預期探測成功或「未啟動」，實得: {other}"),
    }
}

/// ui-language task 3.2：三種探測失敗各自歸入正確代碼，原文照舊。
#[test]
fn probe_reasons_classify_to_stable_codes() {
    fn reason_of(err: RuntimeError) -> String {
        match err {
            RuntimeError::Unavailable { reason, .. } => reason,
            other => panic!("預期 RuntimeError::Unavailable，實得 {other:?}"),
        }
    }

    // 發行版未啟動。
    let outcome = ProbeOutcome {
        status_ok: true,
        stdout: utf16le_bytes("docker-desktop\r\n"),
        stderr: vec![],
    };
    let reason = reason_of(evaluate("Ubuntu-24.04", Ok(outcome), RETRY_AFTER).unwrap_err());
    assert_eq!(reason, "WSL 發行版 Ubuntu-24.04 未啟動");
    let msg = Message::classify(&reason).msg();
    assert_eq!(msg.code, "wsl_distro_not_running");
    assert_eq!(msg.params["distro"], "Ubuntu-24.04");

    // 指令非零結束：detail 是去頭尾空白的 stderr。
    let outcome = ProbeOutcome {
        status_ok: false,
        stdout: vec![],
        stderr: b"  no wsl \n".to_vec(),
    };
    let reason = reason_of(evaluate("Ubuntu-24.04", Ok(outcome), RETRY_AFTER).unwrap_err());
    assert_eq!(reason, "WSL 探測失敗：no wsl");
    let msg = Message::classify(&reason).msg();
    assert_eq!(msg.code, "wsl_probe_failed");
    assert_eq!(msg.params["detail"], "no wsl");

    // 指令跑不起來。
    let io = std::io::Error::other("boom");
    let reason = reason_of(evaluate("Ubuntu-24.04", Err(io), RETRY_AFTER).unwrap_err());
    let msg = Message::classify(&reason).msg();
    assert_eq!(msg.code, "wsl_probe_failed");
    assert_eq!(msg.params["detail"], "boom");
}
