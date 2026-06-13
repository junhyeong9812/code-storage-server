// =============================================================================
// 체크아웃 (checkout.rs)
// =============================================================================
//
// 커밋의 트리를 따라 작업 트리에 파일을 복원하고, 인덱스를 그 상태로 맞춘다.
// pull / clone 후 호출된다.
//
// 파일 위치: crates/cli/src/checkout.rs
// =============================================================================

use std::path::Path;

use anyhow::{Context, Result};
use cts_core::ObjectType;

use crate::index::{Index, IndexEntry};
use crate::objects;
use crate::repo::Repo;

/// 커밋 해시의 스냅샷으로 작업 트리와 인덱스를 복원한다.
pub fn checkout(repo: &Repo, commit_hash: &str) -> Result<()> {
    let commit = objects::read_commit(repo, commit_hash)?;
    let root = repo.root.clone();

    let mut index = Index::new();
    restore_tree(repo, &commit.tree_hash, &root, "", &mut index)?;
    index.save(repo)?;
    Ok(())
}

/// 트리를 재귀적으로 작업 디렉토리에 복원하면서 인덱스를 채운다.
fn restore_tree(
    repo: &Repo,
    tree_hash: &str,
    dir: &Path,
    rel_prefix: &str,
    index: &mut Index,
) -> Result<()> {
    let tree = objects::read_tree(repo, tree_hash)?;
    for entry in tree.entries() {
        let target = dir.join(&entry.name);
        let rel = if rel_prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{rel_prefix}/{}", entry.name)
        };

        match entry.object_type {
            ObjectType::Blob => {
                let (obj_type, content) = objects::read_object(repo, &entry.hash)?;
                if obj_type != "blob" {
                    anyhow::bail!("blob 객체가 아닙니다: {}", entry.hash);
                }
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&target, &content)
                    .with_context(|| format!("파일 복원 실패: {}", target.display()))?;
                set_mode(&target, &entry.mode);

                index.upsert(IndexEntry {
                    path: rel,
                    hash: entry.hash.clone(),
                    mode: entry.mode.clone(),
                    size: content.len() as u64,
                });
            }
            ObjectType::Tree => {
                std::fs::create_dir_all(&target)?;
                restore_tree(repo, &entry.hash, &target, &rel, index)?;
            }
            ObjectType::Commit => {}
        }
    }
    Ok(())
}

/// 실행 모드(100755) 복원
fn set_mode(path: &Path, mode: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if mode == "100755" {
            if let Ok(meta) = std::fs::metadata(path) {
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                let _ = std::fs::set_permissions(path, perm);
            }
        }
    }
    let _ = (path, mode);
}
