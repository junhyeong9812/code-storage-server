// =============================================================================
// Build 엔티티 (build.rs)
// =============================================================================
//
// 하나의 커밋에 대한 CI/CD 빌드 실행 기록.
//
// 파일 위치: crates/server/src/build/domain/entities/build.rs
// =============================================================================

use shared::types::{Id, Timestamp};

use crate::build::domain::value_objects::{BuildId, BuildStatus};

/// 빌드 엔티티
#[derive(Debug, Clone)]
pub struct Build {
    id: BuildId,
    repository_id: Id,
    /// 대상 커밋 해시 (표시용; DB 는 commit_id UUID 로 참조)
    commit_hash: String,
    status: BuildStatus,
    started_at: Option<Timestamp>,
    finished_at: Option<Timestamp>,
    log_path: Option<String>,
    created_at: Timestamp,
}

impl Build {
    #[allow(clippy::too_many_arguments)]
    pub fn from_persistence(
        id: BuildId,
        repository_id: Id,
        commit_hash: String,
        status: BuildStatus,
        started_at: Option<Timestamp>,
        finished_at: Option<Timestamp>,
        log_path: Option<String>,
        created_at: Timestamp,
    ) -> Self {
        Self {
            id,
            repository_id,
            commit_hash,
            status,
            started_at,
            finished_at,
            log_path,
            created_at,
        }
    }

    pub fn id(&self) -> BuildId {
        self.id
    }
    pub fn repository_id(&self) -> Id {
        self.repository_id
    }
    pub fn commit_hash(&self) -> &str {
        &self.commit_hash
    }
    pub fn status(&self) -> BuildStatus {
        self.status
    }
    pub fn started_at(&self) -> Option<Timestamp> {
        self.started_at
    }
    pub fn finished_at(&self) -> Option<Timestamp> {
        self.finished_at
    }
    pub fn log_path(&self) -> Option<&str> {
        self.log_path.as_deref()
    }
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }
}
