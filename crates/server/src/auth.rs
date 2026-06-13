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
use uuid::Uuid;

use crate::error::ApiError;
use crate::repository::application::use_cases::get_repository;
use crate::repository::domain::entities::Repository;
use crate::repository::domain::value_objects::{RepositoryId, Role};
use crate::state::AppState;

/// 인증된 사용자 (Bearer 토큰 필수)
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Id,
    pub username: String,
    /// 토큰 고유 id (로그아웃/철회용)
    pub jti: String,
    /// 토큰 만료 (unix epoch seconds)
    pub exp: i64,
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
        // 철회(로그아웃)된 토큰 거부
        if state.token_revocation.is_revoked(&claims.jti).await? {
            return Err(AppError::Unauthorized.into());
        }
        Ok(AuthUser {
            user_id: claims.user_id,
            username: claims.username,
            jti: claims.jti,
            exp: claims.exp,
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

// -----------------------------------------------------------------------------
// 인가 (repository / build 핸들러 공용)
// -----------------------------------------------------------------------------

/// 사용자의 저장소 접근 수준 (낮음 → 높음)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessLevel {
    None,
    Read,
    Write,
    Admin,
    Owner,
}

/// 저장소 로드 (없으면 404)
pub async fn load_repository(state: &AppState, id: Uuid) -> Result<Repository, ApiError> {
    Ok(get_repository(state.repositories.as_ref(), RepositoryId::from_uuid(id)).await?)
}

/// 사용자(없으면 익명)의 저장소 유효 접근 수준 계산
async fn effective_level(
    state: &AppState,
    repo: &Repository,
    user_id: Option<Id>,
) -> Result<AccessLevel, ApiError> {
    if let Some(uid) = user_id {
        if repo.owner_id().as_uuid() == uid {
            return Ok(AccessLevel::Owner);
        }
        if let Some(role) = state.collaborators.get_role(repo.id(), uid).await? {
            return Ok(match role {
                Role::Read => AccessLevel::Read,
                Role::Write => AccessLevel::Write,
                Role::Admin => AccessLevel::Admin,
            });
        }
    }
    // 익명 또는 비협업자: 공개면 Read, 비공개면 None
    Ok(if repo.is_private() {
        AccessLevel::None
    } else {
        AccessLevel::Read
    })
}

/// 읽기(≥Read): 공개/소유자/협업자. 미달이면 404 은닉.
pub async fn require_read(
    state: &AppState,
    id: Uuid,
    auth: &MaybeAuthUser,
) -> Result<Repository, ApiError> {
    let repo = load_repository(state, id).await?;
    let level = effective_level(state, &repo, auth.0.as_ref().map(|a| a.user_id)).await?;
    if level < AccessLevel::Read {
        return Err(AppError::NotFound(format!("저장소 {id}")).into());
    }
    Ok(repo)
}

/// 쓰기(≥Write): 소유자/admin/write 협업자. 미달이면 403.
pub async fn require_write(
    state: &AppState,
    id: Uuid,
    auth: &AuthUser,
) -> Result<Repository, ApiError> {
    let repo = load_repository(state, id).await?;
    let level = effective_level(state, &repo, Some(auth.user_id)).await?;
    if level < AccessLevel::Write {
        return Err(AppError::Forbidden("저장소 쓰기 권한이 없습니다".into()).into());
    }
    Ok(repo)
}

/// 관리(≥Admin): 소유자/admin 협업자. 협업자 추가·삭제용. 미달이면 403.
pub async fn require_admin(
    state: &AppState,
    id: Uuid,
    auth: &AuthUser,
) -> Result<Repository, ApiError> {
    let repo = load_repository(state, id).await?;
    let level = effective_level(state, &repo, Some(auth.user_id)).await?;
    if level < AccessLevel::Admin {
        return Err(AppError::Forbidden("저장소 관리 권한이 없습니다".into()).into());
    }
    Ok(repo)
}

/// 소유자 전용. 저장소 삭제 등.
pub async fn require_owner(
    state: &AppState,
    id: Uuid,
    auth: &AuthUser,
) -> Result<Repository, ApiError> {
    let repo = load_repository(state, id).await?;
    if repo.owner_id().as_uuid() != auth.user_id {
        return Err(AppError::Forbidden("저장소 소유자가 아닙니다".into()).into());
    }
    Ok(repo)
}
