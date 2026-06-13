// =============================================================================
// Email 값 객체 (email.rs)
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::error::AppError;

/// 검증된 이메일
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email(String);

impl Email {
    /// 간단한 형식 검증 (로컬@도메인.tld)
    pub fn parse(raw: impl Into<String>) -> Result<Self, AppError> {
        let email = raw.into();
        let trimmed = email.trim();
        let valid = trimmed.len() <= 255
            && trimmed.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
                    && !domain.ends_with('.')
                    && !domain.contains(' ')
            });
        if !valid {
            return Err(AppError::InvalidInput(format!("유효하지 않은 이메일: {trimmed}")));
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_and_invalid() {
        assert!(Email::parse("a@b.com").is_ok());
        assert!(Email::parse("x.y@sub.domain.io").is_ok());
        for bad in ["", "noat", "a@b", "@b.com", "a@.com", "a b@c.com"] {
            assert!(Email::parse(bad).is_err(), "{bad} should be invalid");
        }
    }
}
