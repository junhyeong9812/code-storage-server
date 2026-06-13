// =============================================================================
// 객체 번들 (bundle.rs)
// =============================================================================
//
// 로컬 객체 ↔ 와이어 프로토콜(ObjectBundle) 변환.
// - collect_for_push: head 커밋에서 도달 가능한 객체를 의존성 순서로 수집
// - apply_bundle: 받은 번들을 로컬 객체 저장소에 기록
//
// 파일 위치: crates/cli/src/bundle.rs
// =============================================================================

use std::collections::{HashSet, VecDeque};

use anyhow::{bail, Result};
use cts_core::{Commit, ObjectType, TreeEntry};
use shared::protocol::{ObjectBundle, WireBlob, WireCommit, WireTree, WireTreeEntry};

use crate::objects;
use crate::repo::Repo;

/// head 커밋에서 도달 가능한 모든 객체를 수집한다.
///
/// 순서: commits(부모 우선), trees(리프 우선), blobs(임의).
/// 서버 push 저장이 자식부터 필요하므로 이 순서를 지킨다.
pub fn collect_for_push(repo: &Repo, head: &str) -> Result<ObjectBundle> {
    // 커밋 체인 (newest-first → reverse)
    let mut commits: Vec<WireCommit> = Vec::new();
    let mut seen_commit: HashSet<String> = HashSet::new();
    let mut root_trees: Vec<String> = Vec::new();
    let mut current = Some(head.to_string());
    while let Some(hash) = current {
        if !seen_commit.insert(hash.clone()) {
            break;
        }
        let c = objects::read_commit(repo, &hash)?;
        root_trees.push(c.tree_hash.clone());
        let parent = c.parent_hash.clone();
        commits.push(WireCommit {
            hash,
            tree_hash: c.tree_hash,
            parent_hash: c.parent_hash,
            message: c.message,
            author_name: c.author_name,
            author_email: c.author_email,
            timestamp: c.timestamp,
        });
        current = parent;
    }
    commits.reverse();

    // 트리 BFS (root-first) → reverse 로 leaf-first
    let mut trees: Vec<WireTree> = Vec::new();
    let mut tree_seen: HashSet<String> = HashSet::new();
    let mut blob_seen: HashSet<String> = HashSet::new();
    let mut blob_hashes: Vec<String> = Vec::new();
    let mut queue: VecDeque<String> = root_trees.into_iter().collect();
    while let Some(tree_hash) = queue.pop_front() {
        if !tree_seen.insert(tree_hash.clone()) {
            continue;
        }
        let tree = objects::read_tree(repo, &tree_hash)?;
        let mut entries = Vec::with_capacity(tree.entries().len());
        for e in tree.entries() {
            match e.object_type {
                ObjectType::Tree => queue.push_back(e.hash.clone()),
                ObjectType::Blob => {
                    if blob_seen.insert(e.hash.clone()) {
                        blob_hashes.push(e.hash.clone());
                    }
                }
                ObjectType::Commit => {}
            }
            entries.push(WireTreeEntry {
                name: e.name.clone(),
                mode: e.mode.clone(),
                object_type: e.object_type.to_string(),
                hash: e.hash.clone(),
            });
        }
        trees.push(WireTree {
            hash: tree_hash,
            entries,
        });
    }
    trees.reverse();

    // blob 내용
    let mut blobs = Vec::with_capacity(blob_hashes.len());
    for hash in blob_hashes {
        let (obj_type, content) = objects::read_object(repo, &hash)?;
        if obj_type != "blob" {
            bail!("blob 객체가 아닙니다: {hash} ({obj_type})");
        }
        blobs.push(WireBlob { hash, content });
    }

    Ok(ObjectBundle {
        blobs,
        trees,
        commits,
    })
}

/// 받은 번들을 로컬 객체 저장소에 기록한다.
pub fn apply_bundle(repo: &Repo, bundle: &ObjectBundle) -> Result<()> {
    for b in &bundle.blobs {
        objects::write_object(repo, "blob", &b.hash, &b.content)?;
    }
    for t in &bundle.trees {
        let entries: Vec<TreeEntry> = t
            .entries
            .iter()
            .map(|e| {
                Ok(TreeEntry {
                    name: e.name.clone(),
                    object_type: parse_object_type(&e.object_type)?,
                    hash: e.hash.clone(),
                    mode: e.mode.clone(),
                })
            })
            .collect::<Result<_>>()?;
        let body = serde_json::to_vec(&entries)?;
        objects::write_object(repo, "tree", &t.hash, &body)?;
    }
    for c in &bundle.commits {
        let commit = Commit::new(
            c.tree_hash.clone(),
            c.parent_hash.clone(),
            c.message.clone(),
            c.author_name.clone(),
            c.author_email.clone(),
            c.timestamp.clone(),
        );
        let body = serde_json::to_vec(&commit)?;
        objects::write_object(repo, "commit", &c.hash, &body)?;
    }
    Ok(())
}

fn parse_object_type(s: &str) -> Result<ObjectType> {
    match s {
        "blob" => Ok(ObjectType::Blob),
        "tree" => Ok(ObjectType::Tree),
        "commit" => Ok(ObjectType::Commit),
        other => bail!("알 수 없는 객체 종류: {other}"),
    }
}
