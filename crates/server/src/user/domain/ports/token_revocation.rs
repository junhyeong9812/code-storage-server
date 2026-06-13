// =============================================================================
// TokenRevocation 포트 (token_revocation.rs)
// =============================================================================
//
// JWT 철회(로그아웃) 목록. jti 단위로 철회/조회한다.
//
// 파일 위치: crates/server/src/user/domain/ports/token_revocation.rs
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;
use shared::types::Timestamp;

#[async_trait]
pub trait TokenRevocation: Send + Sync {
    /// 해당 jti 가 철회됐는지
    async fn is_revoked(&self, jti: &str) -> Result<bool, AppError>;
    /// jti 철회 (만료 시각은 정리용)
    async fn revoke(&self, jti: &str, expires_at: Timestamp) -> Result<(), AppError>;
}
