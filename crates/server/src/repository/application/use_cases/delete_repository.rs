// =============================================================================
// 유스케이스: 저장소 삭제 (delete_repository.rs)
// =============================================================================

use shared::error::AppError;

use crate::repository::domain::ports::RepositoryRepository;
use crate::repository::domain::value_objects::RepositoryId;

/// 저장소 삭제. 대상이 없으면 NotFound.
pub async fn delete_repository(
    repositories: &dyn RepositoryRepository,
    id: RepositoryId,
) -> Result<(), AppError> {
    let deleted = repositories.delete(id).await?;
    if deleted {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("저장소 {id}")))
    }
}
