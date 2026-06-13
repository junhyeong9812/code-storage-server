// =============================================================================
// UserId 값 객체 (user_id.rs)
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::types::{new_id, Id};

/// 사용자 식별자 (User 바운디드 컨텍스트)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(Id);

impl UserId {
    pub fn generate() -> Self {
        Self(new_id())
    }
    pub fn from_uuid(id: Id) -> Self {
        Self(id)
    }
    pub fn as_uuid(&self) -> Id {
        self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
