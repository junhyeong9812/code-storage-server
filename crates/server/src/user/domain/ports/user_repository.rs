// =============================================================================
// User Repository 포트 (user_repository.rs)
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;

use crate::user::domain::entities::User;
use crate::user::domain::value_objects::UserId;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: &User) -> Result<(), AppError>;
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AppError>;
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, AppError>;
    async fn exists_username(&self, username: &str) -> Result<bool, AppError>;
    async fn exists_email(&self, email: &str) -> Result<bool, AppError>;
}
