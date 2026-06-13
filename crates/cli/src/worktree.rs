// =============================================================================
// 작업 트리 상태 (worktree.rs)
// =============================================================================
//
// 작업 트리 / 인덱스 / HEAD 커밋 트리를 3-way 비교한 상태를 계산한다.
// status 출력과 checkout 의 "변경 사항 존재" 검사에서 공용으로 쓴다.
//
// 파일 위치: crates/cli/src/worktree.rs
// =============================================================================

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use cts_core::{Blob, ObjectType};

use crate::index::Index;
use crate::objects;
use crate::refs;
use crate::repo::{Repo, CTS_DIR};

/// 변경 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    New,
    Modified,
    Deleted,
}

impl ChangeKind {
    pub fn label(&self) -> &'static str {
        match self {
            ChangeKind::New => "새 파일:",
            ChangeKind::Modified => "수정됨: ",
            ChangeKind::Deleted => "삭제됨: ",
        }
    }
}

/// 개별 변경
#[derive(Debug, Clone)]
pub struct Change {
    pub kind: ChangeKind,
    pub path: String,
}

/// 상태 보고서
#[derive(Debug, Clone)]
pub struct StatusReport {
    pub branch: String,
    /// 커밋할 변경 (인덱스 vs HEAD)
    pub staged: Vec<Change>,
    /// 스테이징되지 않은 변경 (작업트리 vs 인덱스)
    pub not_staged: Vec<Change>,
    /// 추적하지 않는 파일
    pub untracked: Vec<String>,
}

impl StatusReport {
    pub fn is_clean(&self) -> bool {
        self.staged.is_empty() && self.not_staged.is_empty() && self.untracked.is_empty()
    }

    /// 커밋되지 않은 변경(스테이징/미스테이징)이 있는가 (untracked 는 제외)
    pub fn has_uncommitted(&self) -> bool {
        !self.staged.is_empty() || !self.not_staged.is_empty()
    }
}

/// 현재 저장소 상태 계산
pub fn compute(repo: &Repo) -> Result<StatusReport> {
    let root = std::fs::canonicalize(&repo.root).context("저장소 루트 경로 해석 실패")?;
    let index = Index::load(repo)?;
    let branch = refs::current_branch(repo)?;

    // HEAD 커밋 트리 (path → blob hash)
    let head = refs::read_branch(repo, &branch)?;
    let committed = match &head {
        Some(h) => flatten_commit(repo, h)?,
        None => BTreeMap::new(),
    };

    // 작업 트리 (path → blob hash)
    let mut working: BTreeMap<String, String> = BTreeMap::new();
    collect_working(&root, &root, &mut working)?;

    // 커밋할 변경 = 인덱스 vs HEAD
    let mut staged = Vec::new();
    for e in &index.entries {
        match committed.get(e.path.as_str()) {
            None => staged.push(Change {
                kind: ChangeKind::New,
                path: e.path.clone(),
            }),
            Some(h) if *h != e.hash => staged.push(Change {
                kind: ChangeKind::Modified,
                path: e.path.clone(),
            }),
            _ => {}
        }
    }
    for path in committed.keys() {
        if index.get(path).is_none() {
            staged.push(Change {
                kind: ChangeKind::Deleted,
                path: path.clone(),
            });
        }
    }

    // 미스테이징 변경 + untracked = 작업트리 vs 인덱스
    let mut not_staged = Vec::new();
    let mut untracked = Vec::new();
    for (path, whash) in &working {
        match index.get(path) {
            None => untracked.push(path.clone()),
            Some(e) if &e.hash != whash => not_staged.push(Change {
                kind: ChangeKind::Modified,
                path: path.clone(),
            }),
            _ => {}
        }
    }
    for e in &index.entries {
        if !working.contains_key(&e.path) {
            not_staged.push(Change {
                kind: ChangeKind::Deleted,
                path: e.path.clone(),
            });
        }
    }

    staged.sort_by(|a, b| a.path.cmp(&b.path));
    not_staged.sort_by(|a, b| a.path.cmp(&b.path));
    untracked.sort();

    Ok(StatusReport {
        branch,
        staged,
        not_staged,
        untracked,
    })
}

/// 커밋의 루트 트리를 평탄화 (path → blob hash)
pub fn flatten_commit(repo: &Repo, commit_hash: &str) -> Result<BTreeMap<String, String>> {
    let commit = objects::read_commit(repo, commit_hash)?;
    let mut map = BTreeMap::new();
    flatten_tree(repo, &commit.tree_hash, "", &mut map)?;
    Ok(map)
}

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
