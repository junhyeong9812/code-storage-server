// =============================================================================
// 인증 추출기 (auth.rs)
// =============================================================================
//
// axum 핸들러에서 `Authorization: Bearer <jwt>` 를 검증해 인증 주체를 얻는다.
// - AuthUser: 인증 필수 (없으면 401)
// - MaybeAuthUser: 인증 선택 (없으면 None) — 공개 읽기용
//
// 파일 위치: crates/server/src/auth.rs
// =============================================================================

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use shared::error::AppError;
use shared::types::Id;

use crate::error::ApiError;
use crate::state::AppState;

/// 인증된 사용자 (Bearer 토큰 필수)
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Id,
    pub username: String,
}

#[axum::async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer(parts).ok_or(AppError::Unauthorized)?;
        let claims = state.tokens.verify(&token)?;
        Ok(AuthUser {
            user_id: claims.user_id,
            username: claims.username,
        })
    }
}

/// 선택적 인증 (토큰이 없거나 잘못돼도 None)
#[derive(Debug, Clone)]
pub struct MaybeAuthUser(pub Option<AuthUser>);

#[axum::async_trait]
impl FromRequestParts<AppState> for MaybeAuthUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(MaybeAuthUser(
            AuthUser::from_request_parts(parts, state).await.ok(),
        ))
    }
}

fn bearer(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|s| s.to_string())
}
