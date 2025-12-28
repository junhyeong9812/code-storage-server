use shared::error::AppError;

/// 저장소 이름 (Value Object)
///
/// 유효성 검사가 포함된 값 객체
/// - 빈 문자열 불가
/// - 100자 제한
/// - 영문, 숫자, 하이픈, 언더스코어만 허용
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryName(String);

impl RepositoryName {
    pub fn  new(name: &str) -> Result<Self, AppError> {
        // 빈 문자열 체크
        if name.is_empty() {
            return Err(AppError::InvalidInput("Repository name cannot be empty".into()));
        }

        // 길이 체크
        if name.len() > 100 {
            return Err(AppError::InvalidInput("Repository name too long (max 100)".into()));
        }

        // 문자 체크: 영문, 숫자, -, _ 만 허용
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(AppError::InvalidInput(
                "Repository name can only contain letters, numbers, hyphens, underscores".into()
            ));
        }

        Ok(Self(name.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RepositoryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}