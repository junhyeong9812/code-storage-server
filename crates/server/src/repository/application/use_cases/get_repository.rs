// =============================================================================
// 유스케이스: 저장소 조회 (get_repository.rs)
// =============================================================================

use shared::error::AppError;

use crate::repository::domain::entities::Repository;
use crate::repository::domain::ports::RepositoryRepository;
use crate::repository::domain::value_objects::RepositoryId;

/// ID로 저장소 조회. 없으면 NotFound.
pub async fn get_repository(
    repositories: &dyn RepositoryRepository,
    id: RepositoryId,
) -> Result<Repository, AppError> {
    repositories
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("저장소 {id}")))
}
