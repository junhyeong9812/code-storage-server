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

use crate::auth::{require_owner, require_read, require_write, AuthUser, MaybeAuthUser};
use crate::error::ApiError;
use crate::repository::application::dto::{
    BlobContentDto, BranchDto, CommitSummary, CreateRepositoryRequest, RepositoryResponse,
    TreeEntryDto,
};
use crate::repository::application::use_cases::{
    browse_tree, create_repository, delete_repository, list_branches, list_commits,
    list_repositories, pull, push, read_blob,
};
use crate::repository::domain::value_objects::{RepositoryId, UserId};
use crate::state::AppState;

/// POST /api/repositories — 저장소 생성 (인증 필요, 소유자=인증 사용자)
pub async fn create_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(request): Json<CreateRepositoryRequest>,
) -> Result<(StatusCode, Json<RepositoryResponse>), ApiError> {
    let owner_id = UserId::from_uuid(auth.user_id);
    let repository = create_repository(state.repositories.as_ref(), owner_id, request).await?;
    Ok((StatusCode::CREATED, Json(repository.into())))
}

/// GET /api/repositories — 저장소 목록 (공개 + 본인 비공개)
pub async fn list_handler(
    State(state): State<AppState>,
    MaybeAuthUser(auth): MaybeAuthUser,
) -> Result<Json<Vec<RepositoryResponse>>, ApiError> {
    let uid = auth.map(|a| a.user_id);
    let repositories = list_repositories(state.repositories.as_ref()).await?;
    let visible = repositories
        .into_iter()
        .filter(|r| !r.is_private() || Some(r.owner_id().as_uuid()) == uid)
        .map(Into::into)
        .collect();
    Ok(Json(visible))
}

/// GET /api/repositories/:id — 저장소 조회 (공개읽기)
pub async fn get_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: MaybeAuthUser,
) -> Result<Json<RepositoryResponse>, ApiError> {
    let repository = require_read(&state, id, &auth).await?;
    Ok(Json(repository.into()))
}

/// DELETE /api/repositories/:id — 저장소 삭제 (소유자만)
pub async fn delete_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: AuthUser,
) -> Result<StatusCode, ApiError> {
    require_owner(&state, id, &auth).await?;
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

/// POST /api/repositories/:id/push — 객체 번들 업로드 + 브랜치 갱신 (쓰기 권한)
pub async fn push_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: AuthUser,
    Json(request): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    require_write(&state, id, &auth).await?;
    let response = push(
        state.objects.as_ref(),
        state.blobs.as_ref(),
        RepositoryId::from_uuid(id),
        request,
    )
    .await?;
    Ok(Json(response))
}

/// GET /api/repositories/:id/pull?branch=main — 객체 번들 다운로드 (공개읽기)
pub async fn pull_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: MaybeAuthUser,
    Query(query): Query<BranchQuery>,
) -> Result<Json<PullResponse>, ApiError> {
    require_read(&state, id, &auth).await?;
    let response = pull(
        state.objects.as_ref(),
        state.blobs.as_ref(),
        RepositoryId::from_uuid(id),
        &query.branch,
    )
    .await?;
    Ok(Json(response))
}

// -----------------------------------------------------------------------------
// 브라우징(읽기) 핸들러 — Web UI 용
// -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CommitsQuery {
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
pub struct TreeQuery {
    #[serde(default)]
    pub path: String,
}

/// GET /api/repositories/:id/branches (공개읽기)
pub async fn branches_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: MaybeAuthUser,
) -> Result<Json<Vec<BranchDto>>, ApiError> {
    require_read(&state, id, &auth).await?;
    let branches = list_branches(state.objects.as_ref(), RepositoryId::from_uuid(id)).await?;
    Ok(Json(branches.into_iter().map(Into::into).collect()))
}

/// GET /api/repositories/:id/commits?branch=&limit= (공개읽기)
pub async fn commits_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: MaybeAuthUser,
    Query(query): Query<CommitsQuery>,
) -> Result<Json<Vec<CommitSummary>>, ApiError> {
    require_read(&state, id, &auth).await?;
    let commits = list_commits(
        state.objects.as_ref(),
        RepositoryId::from_uuid(id),
        &query.branch,
        query.limit,
    )
    .await?;
    Ok(Json(commits.into_iter().map(Into::into).collect()))
}

/// GET /api/repositories/:id/tree/:commit_hash?path= (공개읽기)
pub async fn tree_handler(
    State(state): State<AppState>,
    Path((id, commit_hash)): Path<(Uuid, String)>,
    auth: MaybeAuthUser,
    Query(query): Query<TreeQuery>,
) -> Result<Json<Vec<TreeEntryDto>>, ApiError> {
    require_read(&state, id, &auth).await?;
    let entries = browse_tree(
        state.objects.as_ref(),
        RepositoryId::from_uuid(id),
        &commit_hash,
        &query.path,
    )
    .await?;
    Ok(Json(entries.into_iter().map(Into::into).collect()))
}

/// GET /api/repositories/:id/blob/:hash (공개읽기)
pub async fn blob_handler(
    State(state): State<AppState>,
    Path((id, hash)): Path<(Uuid, String)>,
    auth: MaybeAuthUser,
) -> Result<Json<BlobContentDto>, ApiError> {
    require_read(&state, id, &auth).await?;
    let bytes = read_blob(state.blobs.as_ref(), RepositoryId::from_uuid(id), &hash).await?;
    Ok(Json(BlobContentDto::from_bytes(hash, bytes)))
}
