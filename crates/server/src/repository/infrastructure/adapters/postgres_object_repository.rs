// =============================================================================
// PgObjectRepository (postgres_object_repository.rs)
// =============================================================================
//
// ObjectRepository 포트의 PostgreSQL 구현.
// blob 메타 / tree / tree_entries / commit / branch 테이블을 다룬다.
//
// 핵심: 객체는 해시로 식별하지만 DB 는 내부 UUID 로 참조하므로,
// 자식 해시 → UUID 해석을 어댑터 내부에서 수행한다.
// (자식이 먼저 저장되어 있어야 하며, push 는 의존성 순서로 보낸다)
//
// 파일 위치:
//   crates/server/src/repository/infrastructure/adapters/postgres_object_repository.rs
// =============================================================================

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::repository::domain::ports::{CommitRecord, ObjectRepository, TreeEntryRecord};
use crate::repository::domain::value_objects::RepositoryId;

fn db_err(err: sqlx::Error) -> AppError {
    AppError::Storage(err.to_string())
}

pub struct PgObjectRepository {
    pool: PgPool,
}

impl PgObjectRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 자식 객체(blob/tree)의 해시를 내부 UUID 로 해석
    async fn resolve_child(
        &self,
        repo: RepositoryId,
        object_type: &str,
        hash: &str,
    ) -> Result<Uuid, AppError> {
        let sql = match object_type {
            "blob" => "SELECT id FROM blobs WHERE repository_id = $1 AND hash = $2",
            "tree" => "SELECT id FROM trees WHERE repository_id = $1 AND hash = $2",
            other => {
                return Err(AppError::InvalidInput(format!(
                    "알 수 없는 객체 종류: {other}"
                )))
            }
        };
        let id: Option<Uuid> = sqlx::query_scalar(sql)
            .bind(repo.as_uuid())
            .bind(hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        id.ok_or_else(|| {
            AppError::InvalidInput(format!("참조된 {object_type} 객체가 없습니다: {hash}"))
        })
    }

    async fn tree_id(&self, repo: RepositoryId, hash: &str) -> Result<Uuid, AppError> {
        self.resolve_child(repo, "tree", hash).await
    }

    async fn commit_id(&self, repo: RepositoryId, hash: &str) -> Result<Uuid, AppError> {
        let id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM commits WHERE repository_id = $1 AND hash = $2")
                .bind(repo.as_uuid())
                .bind(hash)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        id.ok_or_else(|| AppError::InvalidInput(format!("커밋이 없습니다: {hash}")))
    }
}

#[derive(sqlx::FromRow)]
struct CommitRow {
    hash: String,
    tree_hash: String,
    parent_hash: Option<String>,
    message: String,
    author_name: String,
    author_email: String,
    committed_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct TreeEntryRow {
    name: String,
    mode: String,
    target_type: String,
    child_hash: Option<String>,
}

#[async_trait]
impl ObjectRepository for PgObjectRepository {
    async fn upsert_blob(
        &self,
        repository_id: RepositoryId,
        hash: &str,
        size: i64,
        storage_path: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            r#"
            INSERT INTO blobs (repository_id, hash, size, storage_path)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (repository_id, hash) DO NOTHING
            "#,
        )
        .bind(repository_id.as_uuid())
        .bind(hash)
        .bind(size)
        .bind(storage_path)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(result.rows_affected() > 0)
    }

    async fn upsert_tree(
        &self,
        repository_id: RepositoryId,
        hash: &str,
        entries: &[TreeEntryRecord],
    ) -> Result<bool, AppError> {
        // 신규 여부 판단
        let existed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM trees WHERE repository_id = $1 AND hash = $2)",
        )
        .bind(repository_id.as_uuid())
        .bind(hash)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;

        // tree 행 확보 (없으면 생성) 후 id 획득
        let tree_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO trees (repository_id, hash)
            VALUES ($1, $2)
            ON CONFLICT (repository_id, hash) DO UPDATE SET hash = EXCLUDED.hash
            RETURNING id
            "#,
        )
        .bind(repository_id.as_uuid())
        .bind(hash)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;

        // 엔트리 재구성 (멱등)
        sqlx::query("DELETE FROM tree_entries WHERE tree_id = $1")
            .bind(tree_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;

        for entry in entries {
            let target_id = self
                .resolve_child(repository_id, &entry.object_type, &entry.child_hash)
                .await?;
            sqlx::query(
                r#"
                INSERT INTO tree_entries (tree_id, name, mode, target_type, target_id)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(tree_id)
            .bind(&entry.name)
            .bind(&entry.mode)
            .bind(&entry.object_type)
            .bind(target_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        }

        Ok(!existed)
    }

    async fn get_tree_entries(
        &self,
        repository_id: RepositoryId,
        hash: &str,
    ) -> Result<Vec<TreeEntryRecord>, AppError> {
        let rows: Vec<TreeEntryRow> = sqlx::query_as(
            r#"
            SELECT te.name,
                   te.mode,
                   te.target_type,
                   COALESCE(b.hash, t.hash) AS child_hash
            FROM trees parent
            JOIN tree_entries te ON te.tree_id = parent.id
            LEFT JOIN blobs b ON te.target_type = 'blob' AND b.id = te.target_id
            LEFT JOIN trees t ON te.target_type = 'tree' AND t.id = te.target_id
            WHERE parent.repository_id = $1 AND parent.hash = $2
            ORDER BY te.name
            "#,
        )
        .bind(repository_id.as_uuid())
        .bind(hash)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        rows.into_iter()
            .map(|r| {
                let child_hash = r.child_hash.ok_or_else(|| {
                    AppError::Storage(format!("트리 엔트리 '{}' 의 대상 해시가 없습니다", r.name))
                })?;
                Ok(TreeEntryRecord {
                    name: r.name,
                    mode: r.mode,
                    object_type: r.target_type,
                    child_hash,
                })
            })
            .collect()
    }

    async fn upsert_commit(
        &self,
        repository_id: RepositoryId,
        commit: &CommitRecord,
    ) -> Result<bool, AppError> {
        let tree_id = self.tree_id(repository_id, &commit.tree_hash).await?;
        let parent_id = match &commit.parent_hash {
            Some(ph) => Some(self.commit_id(repository_id, ph).await?),
            None => None,
        };
        let committed_at = DateTime::parse_from_rfc3339(&commit.timestamp)
            .map_err(|e| AppError::InvalidInput(format!("잘못된 타임스탬프: {e}")))?
            .with_timezone(&Utc);

        let result = sqlx::query(
            r#"
            INSERT INTO commits
                (repository_id, hash, tree_id, parent_id, message,
                 author_name, author_email, committed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (repository_id, hash) DO NOTHING
            "#,
        )
        .bind(repository_id.as_uuid())
        .bind(&commit.hash)
        .bind(tree_id)
        .bind(parent_id)
        .bind(&commit.message)
        .bind(&commit.author_name)
        .bind(&commit.author_email)
        .bind(committed_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_commit(
        &self,
        repository_id: RepositoryId,
        hash: &str,
    ) -> Result<Option<CommitRecord>, AppError> {
        let row: Option<CommitRow> = sqlx::query_as(
            r#"
            SELECT c.hash,
                   t.hash AS tree_hash,
                   p.hash AS parent_hash,
                   c.message,
                   c.author_name,
                   c.author_email,
                   c.committed_at
            FROM commits c
            JOIN trees t ON t.id = c.tree_id
            LEFT JOIN commits p ON p.id = c.parent_id
            WHERE c.repository_id = $1 AND c.hash = $2
            "#,
        )
        .bind(repository_id.as_uuid())
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(row.map(|r| CommitRecord {
            hash: r.hash,
            tree_hash: r.tree_hash,
            parent_hash: r.parent_hash,
            message: r.message,
            author_name: r.author_name,
            author_email: r.author_email,
            timestamp: r.committed_at.to_rfc3339(),
        }))
    }

    async fn set_branch_head(
        &self,
        repository_id: RepositoryId,
        branch: &str,
        commit_hash: &str,
    ) -> Result<(), AppError> {
        let commit_id = self.commit_id(repository_id, commit_hash).await?;
        sqlx::query(
            r#"
            INSERT INTO branches (repository_id, name, head_commit_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (repository_id, name)
            DO UPDATE SET head_commit_id = EXCLUDED.head_commit_id
            "#,
        )
        .bind(repository_id.as_uuid())
        .bind(branch)
        .bind(commit_id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get_branch_head(
        &self,
        repository_id: RepositoryId,
        branch: &str,
    ) -> Result<Option<String>, AppError> {
        let hash: Option<String> = sqlx::query_scalar(
            r#"
            SELECT c.hash
            FROM branches br
            JOIN commits c ON c.id = br.head_commit_id
            WHERE br.repository_id = $1 AND br.name = $2
            "#,
        )
        .bind(repository_id.as_uuid())
        .bind(branch)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(hash)
    }
}
