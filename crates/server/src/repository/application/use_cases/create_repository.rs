// =============================================================================
// 유스케이스: 저장소 생성 (create_repository.rs)
// =============================================================================
//
// 흐름:
// 1. 요청의 이름 문자열을 RepositoryName 으로 검증
// 2. 같은 소유자 + 같은 이름 중복 확인
// 3. Repository 엔티티 생성
// 4. 포트를 통해 저장
//
// 파일 위치: crates/server/src/repository/application/use_cases/create_repository.rs
// =============================================================================

use shared::error::AppError;

use crate::repository::application::dto::CreateRepositoryRequest;
use crate::repository::domain::entities::Repository;
use crate::repository::domain::ports::RepositoryRepository;
use crate::repository::domain::value_objects::{RepositoryName, UserId};

/// 저장소 생성 유스케이스
pub async fn create_repository(
    repositories: &dyn RepositoryRepository,
    owner_id: UserId,
    request: CreateRepositoryRequest,
) -> Result<Repository, AppError> {
    // 1. 이름 검증 (parse, don't validate)
    let name = RepositoryName::parse(request.name)?;

    // 2. 중복 확인
    if repositories
        .exists_by_owner_and_name(owner_id, &name)
        .await?
    {
        return Err(AppError::AlreadyExists(format!(
            "저장소 '{name}' 가 이미 존재합니다"
        )));
    }

    // 3. 엔티티 생성
    let repository = Repository::new(name, request.description, owner_id, request.is_private);

    // 4. 저장
    repositories.create(&repository).await?;

    Ok(repository)
}
