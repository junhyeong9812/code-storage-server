// =============================================================================
// Build DTO (dto/mod.rs)
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::types::{Id, Timestamp};

use crate::build::domain::entities::Build;

/// 빌드 트리거 요청
#[derive(Debug, Deserialize)]
pub struct TriggerBuildRequest {
    /// 빌드할 커밋 해시 (서버에 push 되어 있어야 함)
    pub commit_hash: String,
    /// 실행할 빌드 명령 (생략 시 체크아웃 루트의 cts.build.sh)
    #[serde(default)]
    pub command: Option<String>,
}

/// 빌드 응답
#[derive(Debug, Serialize)]
pub struct BuildResponse {
    pub id: Id,
    pub repository_id: Id,
    pub commit_hash: String,
    pub status: String,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

impl From<Build> for BuildResponse {
    fn from(b: Build) -> Self {
        Self {
            id: b.id().as_uuid(),
            repository_id: b.repository_id(),
            commit_hash: b.commit_hash().to_string(),
            status: b.status().as_str().to_string(),
            started_at: b.started_at(),
            finished_at: b.finished_at(),
            created_at: b.created_at(),
        }
    }
}
