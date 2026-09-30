//! commit hash（`Oid`）的驗證（git-review design D5：「40 或 64 個小寫十六進位字元」）。

use std::fmt;

/// 已驗證的 commit hash：40（SHA-1）或 64（SHA-256）個小寫十六進位字元。
///
/// 只接受小寫——大寫、長度不符、或含任何非十六進位字元（含 `-` 開頭，避免被誤認成
/// git 的旗標）一律拒絕。design Non-Goal：不支援 SHA-256 以外的新雜湊格式，但長度上
/// 已經涵蓋 64 字元，日後若有新格式只需另外檢查內容不需要改這裡的長度判斷。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Oid(String);

/// [`Oid::parse`] 失敗的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OidError {
    /// 長度不是 40 也不是 64。
    BadLength,
    /// 含大寫或非十六進位字元。
    NotLowerHex,
}

impl fmt::Display for OidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OidError::BadLength => write!(f, "commit hash 長度必須是 40 或 64 個字元"),
            OidError::NotLowerHex => write!(f, "commit hash 只能是小寫十六進位字元"),
        }
    }
}

impl std::error::Error for OidError {}

impl Oid {
    /// 驗證並建立一個 `Oid`。
    pub fn parse(raw: &str) -> Result<Oid, OidError> {
        if raw.len() != 40 && raw.len() != 64 {
            return Err(OidError::BadLength);
        }
        if !raw.bytes().all(is_lower_hex_byte) {
            return Err(OidError::NotLowerHex);
        }
        Ok(Oid(raw.to_string()))
    }

    /// 底層字串（給 argv 組裝用）。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_lower_hex_byte(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

#[cfg(test)]
mod tests {
    use super::{Oid, OidError};

    fn hex(n: usize, c: char) -> String {
        std::iter::repeat_n(c, n).collect()
    }

    #[test]
    fn accepts_40_lowercase_hex_chars() {
        let raw = hex(40, 'a');
        let oid = Oid::parse(&raw).expect("40 個小寫十六進位字元應合法");
        assert_eq!(oid.as_str(), raw);
    }

    #[test]
    fn accepts_64_lowercase_hex_chars() {
        let raw = hex(64, 'f');
        let oid = Oid::parse(&raw).expect("64 個小寫十六進位字元應合法");
        assert_eq!(oid.as_str(), raw);
    }

    #[test]
    fn accepts_mixed_digits_and_lowercase_letters() {
        let raw = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(raw.len(), 40, "測試前提：字面值應為 40 字元");
        Oid::parse(raw).expect("數字與 a-f 混合應合法");
    }

    #[test]
    fn rejects_uppercase_hex_chars() {
        let raw = hex(40, 'A');
        assert_eq!(Oid::parse(&raw), Err(OidError::NotLowerHex));
    }

    #[test]
    fn rejects_length_not_40_or_64() {
        for len in [0, 1, 39, 41, 63, 65, 100] {
            let raw = hex(len, 'a');
            assert_eq!(
                Oid::parse(&raw),
                Err(OidError::BadLength),
                "長度 {len} 應被拒絕"
            );
        }
    }

    #[test]
    fn rejects_leading_dash() {
        // `-` 開頭且長度湊到 40：確認不會被誤判成合法值或被當成旗標。
        let raw = format!("-{}", hex(39, 'a'));
        assert_eq!(Oid::parse(&raw), Err(OidError::NotLowerHex));
    }

    #[test]
    fn rejects_non_hex_characters() {
        let raw = format!("g{}", hex(39, 'a'));
        assert_eq!(Oid::parse(&raw), Err(OidError::NotLowerHex));
    }
}
