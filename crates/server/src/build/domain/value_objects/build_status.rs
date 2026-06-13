// =============================================================================
// BuildStatus 값 객체 (build_status.rs)
// =============================================================================
//
// 빌드 생명주기 상태. DB builds.status (VARCHAR) 와 매핑된다.
//
// 파일 위치: crates/server/src/build/domain/value_objects/build_status.rs
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::error::AppError;

/// 빌드 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildStatus {
    /// 대기 중 (생성됨)
    Pending,
    /// 실행 중
    Running,
    /// 성공
    Success,
    /// 실패
    Failed,
}

impl BuildStatus {
    /// DB 저장용 문자열
    pub fn as_str(&self) -> &'static str {
        match self {
            BuildStatus::Pending => "pending",
            BuildStatus::Running => "running",
            BuildStatus::Success => "success",
            BuildStatus::Failed => "failed",
        }
    }

    /// DB 문자열 → enum
    pub fn from_db(s: &str) -> Result<Self, AppError> {
        match s {
            "pending" => Ok(BuildStatus::Pending),
            "running" => Ok(BuildStatus::Running),
            "success" => Ok(BuildStatus::Success),
            "failed" => Ok(BuildStatus::Failed),
            other => Err(AppError::Storage(format!("알 수 없는 빌드 상태: {other}"))),
        }
    }

    /// 종료 상태인지 (success/failed)
    pub fn is_terminal(&self) -> bool {
        matches!(self, BuildStatus::Success | BuildStatus::Failed)
    }
}

impl std::fmt::Display for BuildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_roundtrip() {
        for s in [
            BuildStatus::Pending,
            BuildStatus::Running,
            BuildStatus::Success,
            BuildStatus::Failed,
        ] {
            assert_eq!(BuildStatus::from_db(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn unknown_status_errors() {
        assert!(BuildStatus::from_db("bogus").is_err());
    }

    #[test]
    fn terminal_states() {
        assert!(BuildStatus::Success.is_terminal());
        assert!(BuildStatus::Failed.is_terminal());
        assert!(!BuildStatus::Pending.is_terminal());
        assert!(!BuildStatus::Running.is_terminal());
    }
}
