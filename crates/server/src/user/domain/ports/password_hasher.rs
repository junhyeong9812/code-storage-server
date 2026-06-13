// =============================================================================
// PasswordHasher 포트 (password_hasher.rs)
// =============================================================================
//
// 비밀번호 해싱/검증 인터페이스. 구현은 인프라(BcryptPasswordHasher).
//
// 파일 위치: crates/server/src/user/domain/ports/password_hasher.rs
// =============================================================================

use shared::error::AppError;

pub trait PasswordHasher: Send + Sync {
    /// 평문 비밀번호 → 해시
    fn hash(&self, password: &str) -> Result<String, AppError>;
    /// 평문이 해시와 일치하는지
    fn verify(&self, password: &str, hash: &str) -> Result<bool, AppError>;
}
