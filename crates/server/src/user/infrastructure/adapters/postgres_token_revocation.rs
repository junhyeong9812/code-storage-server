// =============================================================================
// PgTokenRevocation (postgres_token_revocation.rs)
// =============================================================================
//
// revoked_tokens 테이블 기반 JWT 철회 목록.
//
// 파일 위치: crates/server/src/user/infrastructure/adapters/postgres_token_revocation.rs
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;
use shared::types::Timestamp;
use sqlx::PgPool;

use crate::user::domain::ports::TokenRevocation;

fn db_err(err: sqlx::Error) -> AppError {
    AppError::Storage(err.to_string())
}

pub struct PgTokenRevocation {
    pool: PgPool,
}

impl PgTokenRevocation {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenRevocation for PgTokenRevocation {
    async fn is_revoked(&self, jti: &str) -> Result<bool, AppError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revoked_tokens WHERE jti = $1)")
            .bind(jti)
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }

    async fn revoke(&self, jti: &str, expires_at: Timestamp) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO revoked_tokens (jti, expires_at) VALUES ($1, $2) ON CONFLICT (jti) DO NOTHING",
        )
        .bind(jti)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}
