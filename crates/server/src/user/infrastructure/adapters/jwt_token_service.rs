// =============================================================================
// JwtTokenService (jwt_token_service.rs)
// =============================================================================
//
// HS256 JWT 발급/검증. 비밀키는 JWT_SECRET 환경변수에서 주입한다.
//
// 파일 위치: crates/server/src/user/infrastructure/adapters/jwt_token_service.rs
// =============================================================================

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use shared::error::AppError;
use shared::types::Id;
use uuid::Uuid;

use crate::user::domain::ports::{AuthClaims, TokenService};

/// 기본 토큰 만료 (30일)
const DEFAULT_TTL_SECS: i64 = 30 * 24 * 3600;

#[derive(Serialize, Deserialize)]
struct Claims {
    /// subject = user_id (UUID 문자열)
    sub: String,
    username: String,
    /// 만료 (unix epoch seconds)
    exp: usize,
}

pub struct JwtTokenService {
    secret: Vec<u8>,
    ttl_secs: i64,
}

impl JwtTokenService {
    pub fn new(secret: impl Into<Vec<u8>>) -> Self {
        Self {
            secret: secret.into(),
            ttl_secs: DEFAULT_TTL_SECS,
        }
    }
}

impl TokenService for JwtTokenService {
    fn issue(&self, user_id: Id, username: &str) -> Result<String, AppError> {
        let exp = (chrono::Utc::now().timestamp() + self.ttl_secs) as usize;
        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            exp,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&self.secret),
        )
        .map_err(|e| AppError::Internal(format!("토큰 발급 실패: {e}")))
    }

    fn verify(&self, token: &str) -> Result<AuthClaims, AppError> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(&self.secret),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized)?;

        let user_id = Uuid::parse_str(&data.claims.sub).map_err(|_| AppError::Unauthorized)?;
        Ok(AuthClaims {
            user_id,
            username: data.claims.username,
        })
    }
}
