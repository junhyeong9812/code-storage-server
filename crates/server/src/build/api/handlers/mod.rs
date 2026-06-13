// =============================================================================
// Build API 핸들러 (handlers/mod.rs)
// =============================================================================

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::auth::{require_owner, require_read, AuthUser, MaybeAuthUser};
use crate::build::application::dto::{BuildResponse, TriggerBuildRequest};
use crate::build::application::use_cases::{get_build, get_build_log, list_builds, run_build};
use crate::build::domain::value_objects::BuildId;
use crate::error::ApiError;
use crate::state::AppState;

/// POST /api/repositories/:id/builds — 빌드 트리거(실행) (소유자만)
pub async fn trigger_handler(
    State(state): State<AppState>,
    Path(repo_id): Path<Uuid>,
    auth: AuthUser,
    Json(request): Json<TriggerBuildRequest>,
) -> Result<(StatusCode, Json<BuildResponse>), ApiError> {
    require_owner(&state, repo_id, &auth).await?;
    let build = run_build(
        state.builds.as_ref(),
        state.build_runner.as_ref(),
        repo_id,
        &request.commit_hash,
        request.command.as_deref(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(build.into())))
}

/// GET /api/repositories/:id/builds — 빌드 목록 (공개읽기)
pub async fn list_handler(
    State(state): State<AppState>,
    Path(repo_id): Path<Uuid>,
    auth: MaybeAuthUser,
) -> Result<Json<Vec<BuildResponse>>, ApiError> {
    require_read(&state, repo_id, &auth).await?;
    let builds = list_builds(state.builds.as_ref(), repo_id).await?;
    Ok(Json(builds.into_iter().map(Into::into).collect()))
}

/// GET /api/repositories/:id/builds/:build_id — 빌드 상세 (공개읽기)
pub async fn get_handler(
    State(state): State<AppState>,
    Path((repo_id, build_id)): Path<(Uuid, Uuid)>,
    auth: MaybeAuthUser,
) -> Result<Json<BuildResponse>, ApiError> {
    require_read(&state, repo_id, &auth).await?;
    let build = get_build(state.builds.as_ref(), BuildId::from_uuid(build_id)).await?;
    Ok(Json(build.into()))
}

/// GET /api/repositories/:id/builds/:build_id/log — 빌드 로그(텍스트) (공개읽기)
pub async fn log_handler(
    State(state): State<AppState>,
    Path((repo_id, build_id)): Path<(Uuid, Uuid)>,
    auth: MaybeAuthUser,
) -> Result<String, ApiError> {
    require_read(&state, repo_id, &auth).await?;
    let log = get_build_log(state.builds.as_ref(), BuildId::from_uuid(build_id)).await?;
    Ok(log)
}
