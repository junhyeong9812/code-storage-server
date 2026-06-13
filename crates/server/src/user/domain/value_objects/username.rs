// =============================================================================
// Username 값 객체 (username.rs)
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::error::AppError;

/// 검증된 사용자명 (3~50자, 영문/숫자/_/-)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Username(String);

impl Username {
    pub fn parse(raw: impl Into<String>) -> Result<Self, AppError> {
        let name = raw.into();
        let trimmed = name.trim();
        if trimmed.len() < 3 || trimmed.len() > 50 {
            return Err(AppError::InvalidInput("사용자명은 3~50자여야 합니다".into()));
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        {
            return Err(AppError::InvalidInput(
                "사용자명은 영문/숫자/_/- 만 사용할 수 있습니다".into(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_and_invalid() {
        assert!(Username::parse("alice").is_ok());
        assert!(Username::parse("bob_99").is_ok());
        for bad in ["ab", "has space", "bad!", &"x".repeat(51)] {
            assert!(Username::parse(bad).is_err(), "{bad} should be invalid");
        }
    }
}
