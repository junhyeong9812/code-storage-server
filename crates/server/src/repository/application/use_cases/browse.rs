// =============================================================================
// 유스케이스: 브라우징 (browse.rs)
// =============================================================================
//
// Web UI 가 사용하는 읽기 전용 조회.
// - list_branches: 브랜치 목록
// - list_commits: 브랜치 커밋 히스토리(parent 체인)
// - browse_tree: 커밋 트리의 특정 경로 엔트리 목록
// - read_blob: blob 내용
//
// 파일 위치: crates/server/src/repository/application/use_cases/browse.rs
// =============================================================================

use std::collections::HashSet;

use shared::error::AppError;

use crate::repository::domain::ports::{
    BlobStorage, BranchHead, CommitRecord, ObjectRepository, TreeEntryRecord,
};
use crate::repository::domain::value_objects::RepositoryId;

/// 브랜치 목록
pub async fn list_branches(
    objects: &dyn ObjectRepository,
    repo: RepositoryId,
) -> Result<Vec<BranchHead>, AppError> {
    objects.list_branches(repo).await
}

/// 브랜치 커밋 히스토리 (head → parent, 최대 limit개)
pub async fn list_commits(
    objects: &dyn ObjectRepository,
    repo: RepositoryId,
    branch: &str,
    limit: usize,
) -> Result<Vec<CommitRecord>, AppError> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut current = objects.get_branch_head(repo, branch).await?;
    while let Some(hash) = current {
        if out.len() >= limit || !seen.insert(hash.clone()) {
            break;
        }
        let commit = objects
            .get_commit(repo, &hash)
            .await?
            .ok_or_else(|| AppError::Storage(format!("커밋 누락: {hash}")))?;
        current = commit.parent_hash.clone();
        out.push(commit);
    }
    Ok(out)
}

/// 커밋 트리의 특정 경로 엔트리 목록 (path 빈 문자열이면 루트)
pub async fn browse_tree(
    objects: &dyn ObjectRepository,
    repo: RepositoryId,
    commit_hash: &str,
    path: &str,
) -> Result<Vec<TreeEntryRecord>, AppError> {
    let commit = objects
        .get_commit(repo, commit_hash)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("커밋 {commit_hash}")))?;

    let mut tree_hash = commit.tree_hash;
    for part in path.split('/').filter(|s| !s.is_empty()) {
        let entries = objects.get_tree_entries(repo, &tree_hash).await?;
        let next = entries
            .into_iter()
            .find(|e| e.name == part && e.object_type == "tree")
            .ok_or_else(|| AppError::NotFound(format!("디렉토리 없음: {part}")))?;
        tree_hash = next.child_hash;
    }
    objects.get_tree_entries(repo, &tree_hash).await
}

/// blob 내용
pub async fn read_blob(
    blobs: &dyn BlobStorage,
    repo: RepositoryId,
    hash: &str,
) -> Result<Vec<u8>, AppError> {
    blobs.get(repo, hash).await
}
