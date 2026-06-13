// =============================================================================
// User 엔티티 (user.rs)
// =============================================================================

use shared::types::{now, Timestamp};

use crate::user::domain::value_objects::{Email, UserId, Username};

/// 사용자 (애그리거트 루트)
#[derive(Debug, Clone)]
pub struct User {
    id: UserId,
    username: Username,
    email: Email,
    /// bcrypt 해시
    password_hash: String,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl User {
    /// 신규 사용자 생성 (id/타임스탬프 자동)
    pub fn new(username: Username, email: Email, password_hash: String) -> Self {
        let ts = now();
        Self {
            id: UserId::generate(),
            username,
            email,
            password_hash,
            created_at: ts,
            updated_at: ts,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_persistence(
        id: UserId,
        username: Username,
        email: Email,
        password_hash: String,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self {
            id,
            username,
            email,
            password_hash,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> UserId {
        self.id
    }
    pub fn username(&self) -> &Username {
        &self.username
    }
    pub fn email(&self) -> &Email {
        &self.email
    }
    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
}
