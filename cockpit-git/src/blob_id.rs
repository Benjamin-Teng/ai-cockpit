//! `BlobId` 查詢的輸出解析（design D4「BlobId」；Ruling R8，git-review task 3.3 fix round 1）。
//!
//! `rev-parse --verify -q --end-of-options <rev>:<path>` 成功時，stdout 就是物件 hash
//! （40 或 64 個小寫十六進位字元）本身，不需要像其他查詢那樣切欄位；這裡只驗證格式，不猜測
//! 「物件不存在」的語意——**呼叫端（`cockpit`）必須先看 `calls[0]` 是不是
//! `Err(RunnerError::Failed)`**（design D6：那代表這個版本沒有這個檔案，對應 404
//! `not_found_in_rev`，是這個查詢已知的合法非零結束，不是解析錯誤），只有呼叫成功時才呼叫
//! [`BlobId::parse`]（同 `VerifyCommit`／`MergeBase`／`BlobSize` 在 `cockpit` 端的既有分工）。

use crate::parse_error::GitParseError;
use crate::parse_support::single_call;
use crate::query::BlobId;
use crate::runner::{CallOutcome, RunnerError};

/// 物件 hash 的合法形狀：40（SHA-1）或 64（SHA-256）個小寫十六進位字元——`git rev-parse` 的
/// 輸出固定為小寫，大寫視為格式不符預期（縱深防禦，不假設 git 版本／設定不會影響大小寫）。
fn is_object_hash(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl BlobId {
    /// 解析 [`BlobId::commands`] 的唯一一次呼叫（呼叫成功時）：物件 hash。
    pub fn parse(calls: &[Result<CallOutcome, RunnerError>]) -> Result<String, GitParseError> {
        let outcome = single_call(calls)?;
        let text = String::from_utf8_lossy(&outcome.stdout);
        let hash = text.trim();
        if is_object_hash(hash) {
            Ok(hash.to_string())
        } else {
            Err(GitParseError::MalformedOutput(format!(
                "rev-parse --verify 輸出不是合法的物件 hash：{hash:?}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_object_hash;
    use crate::parse_error::GitParseError;
    use crate::query::BlobId;
    use crate::runner::{CallOutcome, RunnerError};

    fn ok_call(stdout: &str) -> Vec<Result<CallOutcome, RunnerError>> {
        vec![Ok(CallOutcome {
            stdout: stdout.as_bytes().to_vec(),
            truncated: false,
        })]
    }

    #[test]
    fn is_object_hash_accepts_40_and_64_lowercase_hex() {
        assert!(is_object_hash(&"a".repeat(40)));
        assert!(is_object_hash(
            &"0123456789abcdef"
                .repeat(3)
                .chars()
                .take(40)
                .collect::<String>()
        ));
        assert!(is_object_hash(&"b".repeat(64)));
    }

    #[test]
    fn is_object_hash_rejects_wrong_length_uppercase_or_non_hex() {
        for bad in [
            "a".repeat(39),
            "a".repeat(41),
            "A".repeat(40),
            "z".repeat(40),
            String::new(),
        ] {
            assert!(!is_object_hash(&bad), "{bad:?} 不應被視為合法物件 hash");
        }
    }

    /// 成功呼叫、輸出恰好是一個合法物件 hash（含結尾換行，`rev-parse` 的實際輸出會帶一個
    /// `\n`）→ 解析成功，換行被裁掉。
    #[test]
    fn parse_extracts_hash_and_trims_trailing_newline() {
        let hash = "a".repeat(40);
        let calls = ok_call(&format!("{hash}\n"));
        assert_eq!(BlobId::parse(&calls), Ok(hash));
    }

    #[test]
    fn parse_accepts_64_char_hash() {
        let hash = "f".repeat(64);
        let calls = ok_call(&format!("{hash}\n"));
        assert_eq!(BlobId::parse(&calls), Ok(hash));
    }

    /// 格式破損（不是 panic）：成功結束卻輸出不像物件 hash 的內容。
    #[test]
    fn parse_rejects_malformed_output_without_panicking() {
        let calls = ok_call("not-a-hash\n");
        match BlobId::parse(&calls) {
            Err(GitParseError::MalformedOutput(_)) => {}
            other => panic!("應為 MalformedOutput，實際：{other:?}"),
        }
    }

    #[test]
    fn parse_rejects_empty_output() {
        let calls = ok_call("");
        match BlobId::parse(&calls) {
            Err(GitParseError::MalformedOutput(_)) => {}
            other => panic!("應為 MalformedOutput，實際：{other:?}"),
        }
    }

    /// 呼叫次數不是 1 → `UnexpectedCallCount`（同其他單次呼叫查詢的既有防禦分支）。
    #[test]
    fn parse_rejects_wrong_call_count() {
        assert_eq!(BlobId::parse(&[]), Err(GitParseError::UnexpectedCallCount));
    }
}
