// =============================================================================
// 유스케이스: Push (push.rs)
// =============================================================================
//
// CLI 가 보낸 객체 번들을 저장하고 브랜치 head 를 갱신한다.
// 저장 순서(의존성): blobs → trees(리프 우선) → commits(부모 우선) → branch.
//
// 파일 위치: crates/server/src/repository/application/use_cases/push.rs
// =============================================================================

use shared::error::AppError;
use shared::protocol::{PushRequest, PushResponse};

use crate::repository::domain::ports::{BlobStorage, CommitRecord, ObjectRepository, TreeEntryRecord};
use crate::repository::domain::value_objects::RepositoryId;

pub async fn push(
    objects: &dyn ObjectRepository,
    blobs: &dyn BlobStorage,
    repository_id: RepositoryId,
    request: PushRequest,
) -> Result<PushResponse, AppError> {
    let mut stored_blobs = 0;
    for blob in &request.objects.blobs {
        let path = blobs.put(repository_id, &blob.hash, &blob.content).await?;
        let is_new = objects
            .upsert_blob(repository_id, &blob.hash, blob.content.len() as i64, &path)
            .await?;
        if is_new {
            stored_blobs += 1;
        }
    }

    let mut stored_trees = 0;
    for tree in &request.objects.trees {
        let entries: Vec<TreeEntryRecord> = tree
            .entries
            .iter()
            .map(|e| TreeEntryRecord {
                name: e.name.clone(),
                mode: e.mode.clone(),
                object_type: e.object_type.clone(),
                child_hash: e.hash.clone(),
            })
            .collect();
        if objects.upsert_tree(repository_id, &tree.hash, &entries).await? {
            stored_trees += 1;
        }
    }

    let mut stored_commits = 0;
    for commit in &request.objects.commits {
        let record = CommitRecord {
            hash: commit.hash.clone(),
            tree_hash: commit.tree_hash.clone(),
            parent_hash: commit.parent_hash.clone(),
            message: commit.message.clone(),
            author_name: commit.author_name.clone(),
            author_email: commit.author_email.clone(),
            timestamp: commit.timestamp.clone(),
        };
        if objects.upsert_commit(repository_id, &record).await? {
            stored_commits += 1;
        }
    }

    objects
        .set_branch_head(repository_id, &request.branch, &request.commit_hash)
        .await?;

    Ok(PushResponse {
        branch: request.branch,
        commit_hash: request.commit_hash,
        stored_blobs,
        stored_trees,
        stored_commits,
    })
}
