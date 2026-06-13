// =============================================================================
// BcryptPasswordHasher (bcrypt_password_hasher.rs)
// =============================================================================

use shared::error::AppError;

use crate::user::domain::ports::PasswordHasher;

pub struct BcryptPasswordHasher;

impl PasswordHasher for BcryptPasswordHasher {
    fn hash(&self, password: &str) -> Result<String, AppError> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("해싱 실패: {e}")))
    }

    fn verify(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        bcrypt::verify(password, hash).map_err(|e| AppError::Internal(format!("검증 실패: {e}")))
    }
}
