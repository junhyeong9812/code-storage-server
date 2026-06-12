// =============================================================================
// Repository Repository 포트 (repository_repository.rs)
// =============================================================================
//
// "포트(Port)"는 도메인이 외부(영속 계층)에 요구하는 인터페이스다.
// 도메인은 이 trait 만 알고, 실제 구현(Postgres 등)은 인프라 레이어에 둔다.
// → 의존성 역전(Dependency Inversion): 도메인이 DB를 모르게 한다.
//
// async fn in trait 은 async-trait 매크로로 구현.
// dyn 호환(object-safe) + Send + Sync 로 두어 AppState 에 Arc<dyn ...> 로 보관.
//
// 파일 위치: crates/server/src/repository/domain/ports/repository_repository.rs
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;

use crate::repository::domain::entities::Repository;
use crate::repository::domain::value_objects::{RepositoryId, RepositoryName, UserId};

/// 저장소 영속화 포트
#[async_trait]
pub trait RepositoryRepository: Send + Sync {
    /// 새 저장소 저장
    async fn create(&self, repository: &Repository) -> Result<(), AppError>;

    /// ID로 저장소 조회 (없으면 None)
    async fn find_by_id(&self, id: RepositoryId) -> Result<Option<Repository>, AppError>;

    /// 모든 저장소 목록 (최신순)
    async fn list(&self) -> Result<Vec<Repository>, AppError>;

    /// 저장소 삭제 (삭제되면 true, 대상이 없으면 false)
    async fn delete(&self, id: RepositoryId) -> Result<bool, AppError>;

    /// 같은 소유자가 같은 이름의 저장소를 이미 가지고 있는지
    async fn exists_by_owner_and_name(
        &self,
        owner_id: UserId,
        name: &RepositoryName,
    ) -> Result<bool, AppError>;
}
