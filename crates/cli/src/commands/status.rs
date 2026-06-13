// =============================================================================
// cts status (commands/status.rs)
// =============================================================================
//
// 작업 트리 / 인덱스 / HEAD 커밋 트리를 3-way 비교해 상태를 보여준다.
//
// - 커밋할 변경 사항: 인덱스 vs HEAD 트리 (새 파일 / 수정 / 삭제)
// - 스테이징되지 않은 변경: 작업 트리 vs 인덱스 (수정 / 삭제)
// - 추적하지 않는 파일: 작업 트리에 있으나 인덱스에 없음
//
// 파일 위치: crates/cli/src/commands/status.rs
// =============================================================================

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use cts_core::{Blob, ObjectType};

use crate::index::Index;
use crate::objects;
use crate::refs;
use crate::repo::{Repo, CTS_DIR};

pub fn run() -> Result<()> {
    let repo = Repo::discover()?;
    let root = std::fs::canonicalize(&repo.root).context("저장소 루트 경로 해석 실패")?;
    let index = Index::load(&repo)?;
    let branch = refs::current_branch(&repo)?;

    println!("브랜치 {branch}");

    // HEAD 커밋 트리 (path → blob hash)
    let head = refs::read_branch(&repo, &branch)?;
    let committed = match &head {
        Some(h) => flatten_commit(&repo, h)?,
        None => BTreeMap::new(),
    };

    // 작업 트리 (path → blob hash)
    let mut working: BTreeMap<String, String> = BTreeMap::new();
    collect_working(&root, &root, &mut working)?;

    // 1) 커밋할 변경 = 인덱스 vs HEAD
    let mut staged: Vec<String> = Vec::new();
    for e in &index.entries {
        match committed.get(e.path.as_str()) {
            None => staged.push(format!("  새 파일:   {}", e.path)),
            Some(h) if *h != e.hash => staged.push(format!("  수정됨:    {}", e.path)),
            _ => {}
        }
    }
    for path in committed.keys() {
        if index.get(path).is_none() {
            staged.push(format!("  삭제됨:    {path}"));
        }
    }

    // 2) 스테이징되지 않은 변경 + 3) 추적하지 않는 파일 = 작업트리 vs 인덱스
    let mut not_staged: Vec<String> = Vec::new();
    let mut untracked: Vec<String> = Vec::new();
    for (path, whash) in &working {
        match index.get(path) {
            None => untracked.push(format!("  {path}")),
            Some(e) if &e.hash != whash => not_staged.push(format!("  수정됨:    {path}")),
            _ => {}
        }
    }
    for e in &index.entries {
        if !working.contains_key(&e.path) {
            not_staged.push(format!("  삭제됨:    {}", e.path));
        }
    }

    staged.sort();
    not_staged.sort();
    untracked.sort();

    if staged.is_empty() && not_staged.is_empty() && untracked.is_empty() {
        println!("커밋할 변경이 없으며 작업 트리가 깨끗합니다.");
        return Ok(());
    }

    if !staged.is_empty() {
        println!("\n커밋할 변경 사항:");
        staged.iter().for_each(|l| println!("{l}"));
    }
    if !not_staged.is_empty() {
        println!("\n스테이징되지 않은 변경:");
        not_staged.iter().for_each(|l| println!("{l}"));
    }
    if !untracked.is_empty() {
        println!("\n추적하지 않는 파일:");
        untracked.iter().for_each(|l| println!("{l}"));
    }
    Ok(())
}

/// 커밋의 루트 트리를 평탄화 (path → blob hash)
fn flatten_commit(repo: &Repo, commit_hash: &str) -> Result<BTreeMap<String, String>> {
    let commit = objects::read_commit(repo, commit_hash)?;
    let mut map = BTreeMap::new();
    flatten_tree(repo, &commit.tree_hash, "", &mut map)?;
    Ok(map)
}

/// 트리를 재귀적으로 평탄화
fn flatten_tree(
    repo: &Repo,
    tree_hash: &str,
    prefix: &str,
    map: &mut BTreeMap<String, String>,
) -> Result<()> {
    let tree = objects::read_tree(repo, tree_hash)?;
    for entry in tree.entries() {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };
        match entry.object_type {
            ObjectType::Blob => {
                map.insert(path, entry.hash.clone());
            }
            ObjectType::Tree => flatten_tree(repo, &entry.hash, &path, map)?,
            ObjectType::Commit => {}
        }
    }
    Ok(())
}

/// 작업 트리의 모든 파일을 해싱해 수집 (.cts 제외)
fn collect_working(root: &Path, dir: &Path, map: &mut BTreeMap<String, String>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("디렉토리를 읽을 수 없습니다: {}", dir.display()))?
    {
        let entry = entry?;
        if entry.file_name() == CTS_DIR {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_working(root, &path, map)?;
        } else {
            let content = std::fs::read(&path)?;
            let mut blob = Blob::new(content);
            let hash = blob.hash().to_string();
            let rel = path
                .strip_prefix(root)
                .with_context(|| format!("저장소 밖의 경로: {}", path.display()))?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            map.insert(rel, hash);
        }
    }
    Ok(())
}
