// =============================================================================
// PgRepositoryRepository (postgres_repository_repository.rs)
// =============================================================================
//
// RepositoryRepository 포트의 PostgreSQL 구현.
//
// sqlx 사용 방식:
// - 컴파일 타임 DB 연결이 필요 없는 "런타임 검증" 쿼리(query_as/query_scalar) 사용.
//   → 빌드 시 DATABASE_URL 이 없어도 컴파일된다. (query! 매크로는 사용하지 않음)
// - DB row 는 RepositoryRow(FromRow)로 받고, 도메인 엔티티로 매핑한다.
//
// 파일 위치:
//   crates/server/src/repository/infrastructure/adapters/postgres_repository_repository.rs
// =============================================================================

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::repository::domain::entities::Repository;
use crate::repository::domain::ports::RepositoryRepository;
use crate::repository::domain::value_objects::{RepositoryId, RepositoryName, UserId};

/// sqlx::Error → AppError 변환 헬퍼
///
/// shared 크레이트는 sqlx 에 의존하지 않으므로(프레임워크 비의존 유지),
/// 변환은 인프라 경계인 이 어댑터에서 처리한다.
fn db_err(err: sqlx::Error) -> AppError {
    AppError::Storage(err.to_string())
}

/// DB의 repositories 행 매핑용 구조체
#[derive(sqlx::FromRow)]
struct RepositoryRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    owner_id: Uuid,
    default_branch: String,
    is_private: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RepositoryRow {
    /// DB row → 도메인 엔티티
    ///
    /// DB 에 저장된 이름이 (어떤 이유로든) 검증을 통과 못 하면 Storage 에러로 취급.
    fn into_entity(self) -> Result<Repository, AppError> {
        let name = RepositoryName::parse(self.name).map_err(|e| {
            AppError::Storage(format!("DB에 저장된 저장소 이름이 유효하지 않음: {e}"))
        })?;
        Ok(Repository::from_persistence(
            RepositoryId::from_uuid(self.id),
            name,
            self.description,
            UserId::from_uuid(self.owner_id),
            self.default_branch,
            self.is_private,
            self.created_at,
            self.updated_at,
        ))
    }
}

/// PostgreSQL 기반 저장소 리포지토리
pub struct PgRepositoryRepository {
    pool: PgPool,
}

impl PgRepositoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RepositoryRepository for PgRepositoryRepository {
    async fn create(&self, repository: &Repository) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO repositories
                (id, name, description, owner_id, default_branch, is_private, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(repository.id().as_uuid())
        .bind(repository.name().as_str())
        .bind(repository.description())
        .bind(repository.owner_id().as_uuid())
        .bind(repository.default_branch())
        .bind(repository.is_private())
        .bind(repository.created_at())
        .bind(repository.updated_at())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(())
    }

    async fn find_by_id(&self, id: RepositoryId) -> Result<Option<Repository>, AppError> {
        let row: Option<RepositoryRow> = sqlx::query_as(
            r#"
            SELECT id, name, description, owner_id, default_branch, is_private, created_at, updated_at
            FROM repositories
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        row.map(RepositoryRow::into_entity).transpose()
    }

    async fn list(&self) -> Result<Vec<Repository>, AppError> {
        let rows: Vec<RepositoryRow> = sqlx::query_as(
            r#"
            SELECT id, name, description, owner_id, default_branch, is_private, created_at, updated_at
            FROM repositories
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        rows.into_iter().map(RepositoryRow::into_entity).collect()
    }

    async fn delete(&self, id: RepositoryId) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM repositories WHERE id = $1")
            .bind(id.as_uuid())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;

        Ok(result.rows_affected() > 0)
    }

    async fn exists_by_owner_and_name(
        &self,
        owner_id: UserId,
        name: &RepositoryName,
    ) -> Result<bool, AppError> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM repositories WHERE owner_id = $1 AND name = $2
            )
            "#,
        )
        .bind(owner_id.as_uuid())
        .bind(name.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(exists)
    }
}
