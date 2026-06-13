// =============================================================================
// Collaborator Repository 포트 (collaborator_repository.rs)
// =============================================================================
//
// 저장소 협업자(역할) 영속화 인터페이스.
//
// 파일 위치: crates/server/src/repository/domain/ports/collaborator_repository.rs
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;
use shared::types::Id;

use crate::repository::domain::value_objects::{RepositoryId, Role};

/// 협업자 레코드 (목록용)
#[derive(Debug, Clone)]
pub struct CollaboratorRecord {
    pub user_id: Id,
    pub username: String,
    pub role: Role,
}

#[async_trait]
pub trait CollaboratorRepository: Send + Sync {
    /// 사용자명으로 협업자 추가/역할 변경 (없는 사용자면 InvalidInput)
    async fn add_by_username(
        &self,
        repository_id: RepositoryId,
        username: &str,
        role: Role,
    ) -> Result<(), AppError>;

    /// 사용자명으로 협업자 제거 (제거되면 true)
    async fn remove_by_username(
        &self,
        repository_id: RepositoryId,
        username: &str,
    ) -> Result<bool, AppError>;

    /// 특정 사용자의 역할 (협업자가 아니면 None)
    async fn get_role(
        &self,
        repository_id: RepositoryId,
        user_id: Id,
    ) -> Result<Option<Role>, AppError>;

    /// 협업자 목록
    async fn list(
        &self,
        repository_id: RepositoryId,
    ) -> Result<Vec<CollaboratorRecord>, AppError>;
}
