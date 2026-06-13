// =============================================================================
// PgBuildRepository (postgres_build_repository.rs)
// =============================================================================
//
// BuildRepository 포트의 PostgreSQL 구현 (builds 테이블).
//
// 파일 위치:
//   crates/server/src/build/infrastructure/adapters/postgres_build_repository.rs
// =============================================================================

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::error::AppError;
use shared::types::{Id, Timestamp};
use sqlx::PgPool;
use uuid::Uuid;

use crate::build::domain::entities::Build;
use crate::build::domain::ports::BuildRepository;
use crate::build::domain::value_objects::{BuildId, BuildStatus};

fn db_err(err: sqlx::Error) -> AppError {
    AppError::Storage(err.to_string())
}

#[derive(sqlx::FromRow)]
struct BuildRow {
    id: Uuid,
    repository_id: Uuid,
    commit_hash: String,
    status: String,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    log_path: Option<String>,
    created_at: DateTime<Utc>,
}

impl BuildRow {
    fn into_entity(self) -> Result<Build, AppError> {
        Ok(Build::from_persistence(
            BuildId::from_uuid(self.id),
            self.repository_id,
            self.commit_hash,
            BuildStatus::from_db(&self.status)?,
            self.started_at,
            self.finished_at,
            self.log_path,
            self.created_at,
        ))
    }
}

const SELECT_BUILD: &str = r#"
    SELECT b.id, b.repository_id, c.hash AS commit_hash, b.status,
           b.started_at, b.finished_at, b.log_path, b.created_at
    FROM builds b
    JOIN commits c ON c.id = b.commit_id
"#;

pub struct PgBuildRepository {
    pool: PgPool,
}

impl PgBuildRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BuildRepository for PgBuildRepository {
    async fn create(&self, repository_id: Id, commit_hash: &str) -> Result<Build, AppError> {
        // 커밋 해시 → commit_id
        let commit_id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM commits WHERE repository_id = $1 AND hash = $2")
                .bind(repository_id)
                .bind(commit_hash)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        let commit_id = commit_id.ok_or_else(|| {
            AppError::InvalidInput(format!("커밋이 서버에 없습니다: {commit_hash} (먼저 push)"))
        })?;

        let id = BuildId::generate();
        let created_at: DateTime<Utc> = sqlx::query_scalar(
            r#"
            INSERT INTO builds (id, repository_id, commit_id, status)
            VALUES ($1, $2, $3, 'pending')
            RETURNING created_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(repository_id)
        .bind(commit_id)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(Build::from_persistence(
            id,
            repository_id,
            commit_hash.to_string(),
            BuildStatus::Pending,
            None,
            None,
            None,
            created_at,
        ))
    }

    async fn find_by_id(&self, id: BuildId) -> Result<Option<Build>, AppError> {
        let row: Option<BuildRow> = sqlx::query_as(&format!("{SELECT_BUILD} WHERE b.id = $1"))
            .bind(id.as_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        row.map(BuildRow::into_entity).transpose()
    }

    async fn list_by_repository(&self, repository_id: Id) -> Result<Vec<Build>, AppError> {
        let rows: Vec<BuildRow> = sqlx::query_as(&format!(
            "{SELECT_BUILD} WHERE b.repository_id = $1 ORDER BY b.created_at DESC"
        ))
        .bind(repository_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter().map(BuildRow::into_entity).collect()
    }

    async fn mark_running(&self, id: BuildId, started_at: Timestamp) -> Result<(), AppError> {
        sqlx::query("UPDATE builds SET status = 'running', started_at = $2 WHERE id = $1")
            .bind(id.as_uuid())
            .bind(started_at)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn mark_finished(
        &self,
        id: BuildId,
        status: BuildStatus,
        finished_at: Timestamp,
        log_path: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE builds SET status = $2, finished_at = $3, log_path = $4 WHERE id = $1",
        )
        .bind(id.as_uuid())
        .bind(status.as_str())
        .bind(finished_at)
        .bind(log_path)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}
