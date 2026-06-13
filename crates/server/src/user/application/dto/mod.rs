// =============================================================================
// User DTO (dto/mod.rs)
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::types::{Id, Timestamp};

use crate::user::domain::entities::User;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 사용자 공개 정보 (비밀번호 제외)
#[derive(Debug, Serialize)]
pub struct UserDto {
    pub id: Id,
    pub username: String,
    pub email: String,
    pub created_at: Timestamp,
}

impl From<User> for UserDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id().as_uuid(),
            username: u.username().as_str().to_string(),
            email: u.email().as_str().to_string(),
            created_at: u.created_at(),
        }
    }
}

/// 인증 응답 (토큰 + 사용자)
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserDto,
}
