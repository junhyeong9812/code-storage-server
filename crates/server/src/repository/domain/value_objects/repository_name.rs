// =============================================================================
// RepositoryName 값 객체 (repository_name.rs)
// =============================================================================
//
// 저장소 이름은 아무 문자열이나 허용하면 안 된다.
// - URL 경로에 들어가므로 안전한 문자만 허용
// - DB 컬럼은 VARCHAR(100)
//
// "검증된 상태로만 존재하는 타입(parse, don't validate)" 패턴:
// RepositoryName 인스턴스가 존재한다 == 이미 유효한 이름이다.
//
// 파일 위치: crates/server/src/repository/domain/value_objects/repository_name.rs
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::error::AppError;

/// 이름 최대 길이 (DB VARCHAR(100)과 일치)
const MAX_LEN: usize = 100;

/// 검증된 저장소 이름
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryName(String);

impl RepositoryName {
    /// 문자열을 검증해 RepositoryName 생성
    ///
    /// # 규칙
    /// - 1자 이상 100자 이하
    /// - 영문자, 숫자, `-`, `_`, `.` 만 허용
    /// - 점(`.`)으로 시작/끝 불가
    pub fn parse(raw: impl Into<String>) -> Result<Self, AppError> {
        let name = raw.into();
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(AppError::InvalidInput("저장소 이름은 비어 있을 수 없습니다".into()));
        }
        if trimmed.len() > MAX_LEN {
            return Err(AppError::InvalidInput(format!(
                "저장소 이름은 최대 {MAX_LEN}자입니다"
            )));
        }
        if trimmed.starts_with('.') || trimmed.ends_with('.') {
            return Err(AppError::InvalidInput(
                "저장소 이름은 '.'으로 시작하거나 끝날 수 없습니다".into(),
            ));
        }
        let valid_chars = trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !valid_chars {
            return Err(AppError::InvalidInput(
                "저장소 이름은 영문/숫자/-/_/. 만 사용할 수 있습니다".into(),
            ));
        }

        Ok(Self(trimmed.to_string()))
    }

    /// 문자열 슬라이스로 접근
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RepositoryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_names() {
        for name in ["my-project", "core_v2", "a", "repo.rs", "ABC123"] {
            assert!(RepositoryName::parse(name).is_ok(), "{name} should be valid");
        }
    }

    #[test]
    fn trims_whitespace() {
        let name = RepositoryName::parse("  hello  ").unwrap();
        assert_eq!(name.as_str(), "hello");
    }

    #[test]
    fn rejects_invalid_names() {
        for name in ["", "   ", ".hidden", "trailing.", "has space", "slash/name"] {
            assert!(RepositoryName::parse(name).is_err(), "{name} should be invalid");
        }
    }

    #[test]
    fn rejects_too_long() {
        let long = "a".repeat(MAX_LEN + 1);
        assert!(RepositoryName::parse(long).is_err());
    }
}
