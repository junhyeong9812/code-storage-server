// =============================================================================
// 유스케이스: Pull (pull.rs)
// =============================================================================
//
// 브랜치 head 에서 도달 가능한 모든 객체(closure)를 모아 번들로 반환한다.
//
// 1. branch head 커밋 해시 조회 (없으면 빈 응답)
// 2. parent 체인을 따라 커밋 수집
// 3. 각 커밋의 루트 트리에서 BFS 로 트리/블롭 해시 수집
// 4. blob 내용 로드
//
// 파일 위치: crates/server/src/repository/application/use_cases/pull.rs
// =============================================================================

use std::collections::{HashSet, VecDeque};

use shared::error::AppError;
use shared::protocol::{ObjectBundle, PullResponse, WireBlob, WireCommit, WireTree, WireTreeEntry};

use crate::repository::domain::ports::{BlobStorage, ObjectRepository};
use crate::repository::domain::value_objects::RepositoryId;

pub async fn pull(
    objects: &dyn ObjectRepository,
    blobs: &dyn BlobStorage,
    repository_id: RepositoryId,
    branch: &str,
) -> Result<PullResponse, AppError> {
    let head = objects.get_branch_head(repository_id, branch).await?;
    let head = match head {
        Some(h) => h,
        None => {
            return Ok(PullResponse {
                branch: branch.to_string(),
                commit_hash: None,
                objects: ObjectBundle::default(),
            })
        }
    };

    // 2. 커밋 체인 수집 (newest-first → 이후 oldest-first 로 뒤집음)
    let mut commits: Vec<WireCommit> = Vec::new();
    let mut root_trees: Vec<String> = Vec::new();
    let mut seen_commit: HashSet<String> = HashSet::new();
    let mut current = Some(head.clone());
    while let Some(hash) = current {
        if !seen_commit.insert(hash.clone()) {
            break;
        }
        let c = objects
            .get_commit(repository_id, &hash)
            .await?
            .ok_or_else(|| AppError::Storage(format!("커밋 객체 누락: {hash}")))?;
        root_trees.push(c.tree_hash.clone());
        let parent = c.parent_hash.clone();
        commits.push(WireCommit {
            hash: c.hash,
            tree_hash: c.tree_hash,
            parent_hash: c.parent_hash,
            message: c.message,
            author_name: c.author_name,
            author_email: c.author_email,
            timestamp: c.timestamp,
        });
        current = parent;
    }
    commits.reverse(); // 부모 우선

    // 3. 트리/블롭 BFS 수집
    let mut trees: Vec<WireTree> = Vec::new();
    let mut tree_seen: HashSet<String> = HashSet::new();
    let mut blob_seen: HashSet<String> = HashSet::new();
    let mut blob_hashes: Vec<String> = Vec::new();

    let mut queue: VecDeque<String> = root_trees.into_iter().collect();
    while let Some(tree_hash) = queue.pop_front() {
        if !tree_seen.insert(tree_hash.clone()) {
            continue;
        }
        let entries = objects.get_tree_entries(repository_id, &tree_hash).await?;
        let mut wire_entries = Vec::with_capacity(entries.len());
        for e in entries {
            match e.object_type.as_str() {
                "tree" => queue.push_back(e.child_hash.clone()),
                "blob" => {
                    if blob_seen.insert(e.child_hash.clone()) {
                        blob_hashes.push(e.child_hash.clone());
                    }
                }
                _ => {}
            }
            wire_entries.push(WireTreeEntry {
                name: e.name,
                mode: e.mode,
                object_type: e.object_type,
                hash: e.child_hash,
            });
        }
        trees.push(WireTree {
            hash: tree_hash,
            entries: wire_entries,
        });
    }

    // 4. blob 내용 로드
    let mut wire_blobs = Vec::with_capacity(blob_hashes.len());
    for hash in blob_hashes {
        let content = blobs.get(repository_id, &hash).await?;
        wire_blobs.push(WireBlob { hash, content });
    }

    Ok(PullResponse {
        branch: branch.to_string(),
        commit_hash: Some(head),
        objects: ObjectBundle {
            blobs: wire_blobs,
            trees,
            commits,
        },
    })
}
