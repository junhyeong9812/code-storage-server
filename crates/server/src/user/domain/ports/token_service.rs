// =============================================================================
// TokenService 포트 (token_service.rs)
// =============================================================================
//
// 인증 토큰 발급/검증 인터페이스. 구현은 인프라(JwtTokenService).
//
// 파일 위치: crates/server/src/user/domain/ports/token_service.rs
// =============================================================================

use shared::error::AppError;
use shared::types::Id;

/// 토큰에서 복원한 인증 주체
#[derive(Debug, Clone)]
pub struct AuthClaims {
    pub user_id: Id,
    pub username: String,
}

pub trait TokenService: Send + Sync {
    /// 사용자에 대한 토큰 발급
    fn issue(&self, user_id: Id, username: &str) -> Result<String, AppError>;
    /// 토큰 검증 → 인증 주체
    fn verify(&self, token: &str) -> Result<AuthClaims, AppError>;
}
