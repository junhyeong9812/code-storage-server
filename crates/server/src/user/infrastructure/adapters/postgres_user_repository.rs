// =============================================================================
// PgUserRepository (postgres_user_repository.rs)
// =============================================================================

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::user::domain::entities::User;
use crate::user::domain::ports::UserRepository;
use crate::user::domain::value_objects::{Email, UserId, Username};

fn db_err(err: sqlx::Error) -> AppError {
    AppError::Storage(err.to_string())
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    email: String,
    password_hash: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl UserRow {
    fn into_entity(self) -> Result<User, AppError> {
        Ok(User::from_persistence(
            UserId::from_uuid(self.id),
            Username::parse(self.username)?,
            Email::parse(self.email)?,
            self.password_hash,
            self.created_at,
            self.updated_at,
        ))
    }
}

const SELECT_USER: &str =
    "SELECT id, username, email, password_hash, created_at, updated_at FROM users";

pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn create(&self, user: &User) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO users (id, username, email, password_hash, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(user.id().as_uuid())
        .bind(user.username().as_str())
        .bind(user.email().as_str())
        .bind(user.password_hash())
        .bind(user.created_at())
        .bind(user.updated_at())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        let row: Option<UserRow> = sqlx::query_as(&format!("{SELECT_USER} WHERE username = $1"))
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        row.map(UserRow::into_entity).transpose()
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, AppError> {
        let row: Option<UserRow> = sqlx::query_as(&format!("{SELECT_USER} WHERE id = $1"))
            .bind(id.as_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        row.map(UserRow::into_entity).transpose()
    }

    async fn exists_username(&self, username: &str) -> Result<bool, AppError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
            .bind(username)
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }

    async fn exists_email(&self, email: &str) -> Result<bool, AppError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
            .bind(email)
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }
}
