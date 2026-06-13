// =============================================================================
// 유스케이스: 협업자 관리 (collaborators.rs)
// =============================================================================

use shared::error::AppError;

use crate::repository::domain::ports::{CollaboratorRecord, CollaboratorRepository};
use crate::repository::domain::value_objects::{RepositoryId, Role};

pub async fn add_collaborator(
    collaborators: &dyn CollaboratorRepository,
    repo: RepositoryId,
    username: &str,
    role: Role,
) -> Result<(), AppError> {
    collaborators.add_by_username(repo, username, role).await
}

pub async fn remove_collaborator(
    collaborators: &dyn CollaboratorRepository,
    repo: RepositoryId,
    username: &str,
) -> Result<(), AppError> {
    if collaborators.remove_by_username(repo, username).await? {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("협업자 {username}")))
    }
}

pub async fn list_collaborators(
    collaborators: &dyn CollaboratorRepository,
    repo: RepositoryId,
) -> Result<Vec<CollaboratorRecord>, AppError> {
    collaborators.list(repo).await
}
