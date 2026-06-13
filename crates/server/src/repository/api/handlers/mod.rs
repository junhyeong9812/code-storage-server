// =============================================================================
// Repository API 핸들러 (handlers/mod.rs)
// =============================================================================
//
// axum 핸들러: HTTP 요청을 받아 유스케이스를 호출하고 응답으로 변환한다.
// - 비즈니스 로직은 유스케이스에, 핸들러는 "변환/배선"만 담당.
// - 에러는 ApiError 로 전파 → 자동 HTTP 상태 코드 매핑.
//
// 인증(Phase: User) 도입 전까지 owner 는 시드 유저로 고정한다.
//
// 파일 위치: crates/server/src/repository/api/handlers/mod.rs
// =============================================================================

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use shared::protocol::{PullResponse, PushRequest, PushResponse};
use uuid::Uuid;

use crate::error::ApiError;
use crate::repository::application::dto::{CreateRepositoryRequest, RepositoryResponse};
use crate::repository::application::use_cases::{
    create_repository, delete_repository, get_repository, list_repositories, pull, push,
};
use crate::repository::domain::value_objects::{RepositoryId, UserId};
use crate::state::AppState;

/// 시드 테스트 유저(`00000000-0000-0000-0000-000000000001`).
///
/// TODO(Phase User): 인증 도입 시 요청에서 인증된 사용자 ID로 교체.
const SEEDED_OWNER_ID: Uuid = Uuid::from_u128(1);

/// POST /api/repositories — 저장소 생성
pub async fn create_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateRepositoryRequest>,
) -> Result<(StatusCode, Json<RepositoryResponse>), ApiError> {
    let owner_id = UserId::from_uuid(SEEDED_OWNER_ID);
    let repository = create_repository(state.repositories.as_ref(), owner_id, request).await?;
    Ok((StatusCode::CREATED, Json(repository.into())))
}

/// GET /api/repositories — 저장소 목록
pub async fn list_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<RepositoryResponse>>, ApiError> {
    let repositories = list_repositories(state.repositories.as_ref()).await?;
    Ok(Json(repositories.into_iter().map(Into::into).collect()))
}

/// GET /api/repositories/:id — 저장소 조회
pub async fn get_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RepositoryResponse>, ApiError> {
    let repository =
        get_repository(state.repositories.as_ref(), RepositoryId::from_uuid(id)).await?;
    Ok(Json(repository.into()))
}

/// DELETE /api/repositories/:id — 저장소 삭제
pub async fn delete_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    delete_repository(state.repositories.as_ref(), RepositoryId::from_uuid(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 브랜치 쿼리 파라미터 (?branch=main)
#[derive(Debug, Deserialize)]
pub struct BranchQuery {
    #[serde(default = "default_branch")]
    pub branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

/// POST /api/repositories/:id/push — 객체 번들 업로드 + 브랜치 갱신
pub async fn push_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    ensure_repo_exists(&state, id).await?;
    let response = push(
        state.objects.as_ref(),
        state.blobs.as_ref(),
        RepositoryId::from_uuid(id),
        request,
    )
    .await?;
    Ok(Json(response))
}

/// GET /api/repositories/:id/pull?branch=main — 객체 번들 다운로드
pub async fn pull_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<BranchQuery>,
) -> Result<Json<PullResponse>, ApiError> {
    ensure_repo_exists(&state, id).await?;
    let response = pull(
        state.objects.as_ref(),
        state.blobs.as_ref(),
        RepositoryId::from_uuid(id),
        &query.branch,
    )
    .await?;
    Ok(Json(response))
}

/// 저장소 존재 확인 (없으면 NotFound → 404)
async fn ensure_repo_exists(state: &AppState, id: Uuid) -> Result<(), ApiError> {
    get_repository(state.repositories.as_ref(), RepositoryId::from_uuid(id)).await?;
    Ok(())
}
