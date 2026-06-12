// =============================================================================
// 유스케이스: 저장소 목록 (list_repositories.rs)
// =============================================================================

use shared::error::AppError;

use crate::repository::domain::entities::Repository;
use crate::repository::domain::ports::RepositoryRepository;

/// 모든 저장소를 최신순으로 반환.
///
/// (인증 도입 후에는 소유자/공개 여부로 필터링하도록 확장 예정)
pub async fn list_repositories(
    repositories: &dyn RepositoryRepository,
) -> Result<Vec<Repository>, AppError> {
    repositories.list().await
}
