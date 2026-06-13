// =============================================================================
// Role 값 객체 (role.rs)
// =============================================================================
//
// 협업자 역할. read < write < admin (owner 는 별도 최상위).
//
// 파일 위치: crates/server/src/repository/domain/value_objects/role.rs
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Read,
    Write,
    Admin,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Read => "read",
            Role::Write => "write",
            Role::Admin => "admin",
        }
    }

    pub fn from_db(s: &str) -> Result<Self, AppError> {
        match s {
            "read" => Ok(Role::Read),
            "write" => Ok(Role::Write),
            "admin" => Ok(Role::Admin),
            other => Err(AppError::InvalidInput(format!("알 수 없는 역할: {other}"))),
        }
    }

    /// 권한 수준 (높을수록 강함)
    pub fn level(&self) -> u8 {
        match self {
            Role::Read => 1,
            Role::Write => 2,
            Role::Admin => 3,
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
