// =============================================================================
// PgCollaboratorRepository (postgres_collaborator_repository.rs)
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;
use shared::types::Id;
use sqlx::PgPool;
use uuid::Uuid;

use crate::repository::domain::ports::{CollaboratorRecord, CollaboratorRepository};
use crate::repository::domain::value_objects::{RepositoryId, Role};

fn db_err(err: sqlx::Error) -> AppError {
    AppError::Storage(err.to_string())
}

pub struct PgCollaboratorRepository {
    pool: PgPool,
}

impl PgCollaboratorRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn user_id_of(&self, username: &str) -> Result<Uuid, AppError> {
        let id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE username = $1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        id.ok_or_else(|| AppError::InvalidInput(format!("사용자가 없습니다: {username}")))
    }
}

#[async_trait]
impl CollaboratorRepository for PgCollaboratorRepository {
    async fn add_by_username(
        &self,
        repository_id: RepositoryId,
        username: &str,
        role: Role,
    ) -> Result<(), AppError> {
        let user_id = self.user_id_of(username).await?;
        sqlx::query(
            r#"
            INSERT INTO repository_collaborators (repository_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (repository_id, user_id) DO UPDATE SET role = EXCLUDED.role
            "#,
        )
        .bind(repository_id.as_uuid())
        .bind(user_id)
        .bind(role.as_str())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn remove_by_username(
        &self,
        repository_id: RepositoryId,
        username: &str,
    ) -> Result<bool, AppError> {
        let user_id = self.user_id_of(username).await?;
        let result = sqlx::query(
            "DELETE FROM repository_collaborators WHERE repository_id = $1 AND user_id = $2",
        )
        .bind(repository_id.as_uuid())
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    async fn get_role(
        &self,
        repository_id: RepositoryId,
        user_id: Id,
    ) -> Result<Option<Role>, AppError> {
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM repository_collaborators WHERE repository_id = $1 AND user_id = $2",
        )
        .bind(repository_id.as_uuid())
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        role.map(|r| Role::from_db(&r)).transpose()
    }

    async fn list(
        &self,
        repository_id: RepositoryId,
    ) -> Result<Vec<CollaboratorRecord>, AppError> {
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT rc.user_id, u.username, rc.role
            FROM repository_collaborators rc
            JOIN users u ON u.id = rc.user_id
            WHERE rc.repository_id = $1
            ORDER BY u.username
            "#,
        )
        .bind(repository_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        rows.into_iter()
            .map(|(user_id, username, role)| {
                Ok(CollaboratorRecord {
                    user_id,
                    username,
                    role: Role::from_db(&role)?,
                })
            })
            .collect()
    }
}
