// =============================================================================
// User 유스케이스 (use_cases/mod.rs)
// =============================================================================
//
// register / login. 둘 다 토큰을 발급해 AuthResponse 로 반환한다.
//
// 파일 위치: crates/server/src/user/application/use_cases/mod.rs
// =============================================================================

use shared::error::AppError;

use crate::user::application::dto::{AuthResponse, LoginRequest, RegisterRequest};
use crate::user::domain::entities::User;
use crate::user::domain::ports::{PasswordHasher, TokenService, UserRepository};
use crate::user::domain::value_objects::{Email, Username};

/// 회원가입
pub async fn register(
    users: &dyn UserRepository,
    hasher: &dyn PasswordHasher,
    tokens: &dyn TokenService,
    request: RegisterRequest,
) -> Result<AuthResponse, AppError> {
    let username = Username::parse(request.username)?;
    let email = Email::parse(request.email)?;
    if request.password.len() < 6 {
        return Err(AppError::InvalidInput("비밀번호는 6자 이상이어야 합니다".into()));
    }

    if users.exists_username(username.as_str()).await? {
        return Err(AppError::AlreadyExists(format!("사용자명 '{username}'")));
    }
    if users.exists_email(email.as_str()).await? {
        return Err(AppError::AlreadyExists(format!("이메일 '{email}'")));
    }

    let password_hash = hasher.hash(&request.password)?;
    let user = User::new(username, email, password_hash);
    users.create(&user).await?;

    let token = tokens.issue(user.id().as_uuid(), user.username().as_str())?;
    Ok(AuthResponse {
        token,
        user: user.into(),
    })
}

/// 로그인
pub async fn login(
    users: &dyn UserRepository,
    hasher: &dyn PasswordHasher,
    tokens: &dyn TokenService,
    request: LoginRequest,
) -> Result<AuthResponse, AppError> {
    // 사용자명/비밀번호 어느 쪽이 틀렸는지 노출하지 않도록 동일 에러 사용
    let invalid = || AppError::Unauthorized;

    let user = users
        .find_by_username(&request.username)
        .await?
        .ok_or_else(invalid)?;

    if !hasher.verify(&request.password, user.password_hash())? {
        return Err(invalid());
    }

    let token = tokens.issue(user.id().as_uuid(), user.username().as_str())?;
    Ok(AuthResponse {
        token,
        user: user.into(),
    })
}
