// =============================================================================
// User API 핸들러 (handlers/mod.rs)
// =============================================================================

use axum::{extract::State, http::StatusCode, Json};
use shared::error::AppError;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;
use crate::user::application::dto::{AuthResponse, LoginRequest, RegisterRequest, UserDto};
use crate::user::application::use_cases::{login, logout, register};
use crate::user::domain::value_objects::UserId;

/// POST /api/auth/register
pub async fn register_handler(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let resp = register(
        state.users.as_ref(),
        state.password_hasher.as_ref(),
        state.tokens.as_ref(),
        req,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(resp)))
}

/// POST /api/auth/login
pub async fn login_handler(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let resp = login(
        state.users.as_ref(),
        state.password_hasher.as_ref(),
        state.tokens.as_ref(),
        req,
    )
    .await?;
    Ok(Json(resp))
}

/// POST /api/auth/logout — 현재 토큰 철회 (인증 필요)
pub async fn logout_handler(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<StatusCode, ApiError> {
    logout(state.token_revocation.as_ref(), &auth.jti, auth.exp).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/users/me (인증 필요)
pub async fn me_handler(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<UserDto>, ApiError> {
    let user = state
        .users
        .find_by_id(UserId::from_uuid(auth.user_id))
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(Json(user.into()))
}
