// =============================================================================
// Build Repository 포트 (build_repository.rs)
// =============================================================================
//
// 빌드 기록의 영속화 인터페이스.
//
// 파일 위치: crates/server/src/build/domain/ports/build_repository.rs
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;
use shared::types::{Id, Timestamp};

use crate::build::domain::entities::Build;
use crate::build::domain::value_objects::{BuildId, BuildStatus};

/// 빌드 영속화 포트
#[async_trait]
pub trait BuildRepository: Send + Sync {
    /// 커밋 해시로 새 빌드를 생성한다(pending). 커밋 해시 → commit_id 해석 포함.
    async fn create(&self, repository_id: Id, commit_hash: &str) -> Result<Build, AppError>;

    async fn find_by_id(&self, id: BuildId) -> Result<Option<Build>, AppError>;

    async fn list_by_repository(&self, repository_id: Id) -> Result<Vec<Build>, AppError>;

    /// 실행 시작 표시
    async fn mark_running(&self, id: BuildId, started_at: Timestamp) -> Result<(), AppError>;

    /// 종료 표시 (success/failed + 로그 경로)
    async fn mark_finished(
        &self,
        id: BuildId,
        status: BuildStatus,
        finished_at: Timestamp,
        log_path: &str,
    ) -> Result<(), AppError>;
}
