// =============================================================================
// Object Repository 포트 (object_repository.rs)
// =============================================================================
//
// blob 메타 / tree / commit / branch 의 그래프를 DB 에 저장·조회하는 포트.
// 객체는 해시로 식별하며, hash ↔ 내부 UUID 해석은 어댑터가 담당한다.
//
// 파일 위치: crates/server/src/repository/domain/ports/object_repository.rs
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;

use crate::repository::domain::value_objects::RepositoryId;

/// 트리 엔트리 (저장/조회 공용)
#[derive(Debug, Clone)]
pub struct TreeEntryRecord {
    pub name: String,
    /// 파일 모드 ("100644" / "100755" / "040000")
    pub mode: String,
    /// 참조 종류 ("blob" | "tree")
    pub object_type: String,
    /// 참조 대상 객체 해시
    pub child_hash: String,
}

/// 브랜치 head (이름 + 커밋 해시)
#[derive(Debug, Clone)]
pub struct BranchHead {
    pub name: String,
    pub commit_hash: String,
}

/// 커밋 레코드 (저장/조회 공용)
#[derive(Debug, Clone)]
pub struct CommitRecord {
    pub hash: String,
    pub tree_hash: String,
    pub parent_hash: Option<String>,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    /// RFC3339 타임스탬프
    pub timestamp: String,
}

/// 객체 그래프 영속화 포트
#[async_trait]
pub trait ObjectRepository: Send + Sync {
    // --- blob 메타 ---
    async fn upsert_blob(
        &self,
        repository_id: RepositoryId,
        hash: &str,
        size: i64,
        storage_path: &str,
    ) -> Result<bool, AppError>; // 새로 저장했으면 true, 이미 있었으면 false

    // --- tree ---
    /// 트리와 엔트리들을 저장한다. 엔트리의 child 는 이미 저장되어 있어야 한다.
    async fn upsert_tree(
        &self,
        repository_id: RepositoryId,
        hash: &str,
        entries: &[TreeEntryRecord],
    ) -> Result<bool, AppError>;

    async fn get_tree_entries(
        &self,
        repository_id: RepositoryId,
        hash: &str,
    ) -> Result<Vec<TreeEntryRecord>, AppError>;

    // --- commit ---
    async fn upsert_commit(
        &self,
        repository_id: RepositoryId,
        commit: &CommitRecord,
    ) -> Result<bool, AppError>;

    async fn get_commit(
        &self,
        repository_id: RepositoryId,
        hash: &str,
    ) -> Result<Option<CommitRecord>, AppError>;

    // --- branch ---
    async fn set_branch_head(
        &self,
        repository_id: RepositoryId,
        branch: &str,
        commit_hash: &str,
    ) -> Result<(), AppError>;

    async fn get_branch_head(
        &self,
        repository_id: RepositoryId,
        branch: &str,
    ) -> Result<Option<String>, AppError>;

    /// 저장소의 모든 브랜치 (이름 + head 커밋 해시)
    async fn list_branches(
        &self,
        repository_id: RepositoryId,
    ) -> Result<Vec<BranchHead>, AppError>;
}
