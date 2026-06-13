// =============================================================================
// BuildId 값 객체 (build_id.rs)
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::types::{new_id, Id};

/// 빌드 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildId(Id);

impl BuildId {
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

impl std::fmt::Display for BuildId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
