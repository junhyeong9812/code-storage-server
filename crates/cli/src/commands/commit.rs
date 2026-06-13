// =============================================================================
// cts commit (commands/commit.rs)
// =============================================================================
//
// 스테이징된 파일들로 커밋을 생성한다.
//   cts commit -m "메시지"
//
// 흐름:
// 1. 인덱스 → 중첩 Tree 구조 빌드 (디렉토리마다 별도 tree 객체)
// 2. 루트 tree 해시 확정
// 3. 현재 브랜치의 head 를 parent 로 Commit 생성
// 4. 브랜치 head 를 새 커밋으로 갱신
//
// 파일 위치: crates/cli/src/commands/commit.rs
// =============================================================================

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use cts_core::{Commit, Tree, TreeEntry};

use crate::config::Config;
use crate::index::Index;
use crate::objects;
use crate::refs;
use crate::repo::Repo;

pub fn run(message: String) -> Result<()> {
    if message.trim().is_empty() {
        bail!("커밋 메시지가 비어 있습니다 (-m \"메시지\")");
    }

    let repo = Repo::discover()?;
    let index = Index::load(&repo)?;
    if index.entries.is_empty() {
        bail!("커밋할 변경이 없습니다 (스테이징된 파일 없음). 'cts add' 를 먼저 실행하세요.");
    }

    let config = Config::load(&repo)?;
    let branch = refs::current_branch(&repo)?;
    let parent = refs::read_branch(&repo, &branch)?;

    // 1~2. 인덱스 → 중첩 트리 → 루트 해시
    let root = build_root_tree(&repo, &index)?;

    // 3. 커밋 객체 생성
    let timestamp = shared::types::now().to_rfc3339();
    let mut commit = Commit::new(
        root.clone(),
        parent.clone(),
        message.clone(),
        config.author_name,
        config.author_email,
        timestamp,
    );
    let commit_hash = objects::write_commit(&repo, &mut commit)?;

    // 4. 브랜치 head 갱신
    refs::update_branch(&repo, &branch, &commit_hash)?;

    let label = if parent.is_none() { " (root-commit)" } else { "" };
    let summary = message.lines().next().unwrap_or("");
    println!("[{branch} {}]{label} {summary}", short(&commit_hash));
    println!(
        "  {}개 파일, tree {}",
        index.entries.len(),
        short(&root)
    );
    Ok(())
}

/// 해시 앞 10자 (표시용)
fn short(hash: &str) -> &str {
    &hash[..hash.len().min(10)]
}

// -----------------------------------------------------------------------------
// 중첩 트리 빌더
// -----------------------------------------------------------------------------

/// 디렉토리 트리 노드 (빌드용 중간 구조)
#[derive(Default)]
struct TreeNode {
    /// 이 디렉토리 바로 아래의 파일들: name → (hash, mode)
    files: BTreeMap<String, (String, String)>,
    /// 하위 디렉토리: name → TreeNode
    dirs: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    /// 경로 컴포넌트들을 따라 파일을 삽입
    fn insert(&mut self, parts: &[String], hash: &str, mode: &str) {
        match parts {
            [] => {}
            [file] => {
                self.files
                    .insert(file.clone(), (hash.to_string(), mode.to_string()));
            }
            [dir, rest @ ..] => {
                self.dirs
                    .entry(dir.clone())
                    .or_default()
                    .insert(rest, hash, mode);
            }
        }
    }
}

/// 인덱스로부터 루트 트리를 빌드하고, 모든 tree 객체를 저장한 뒤 루트 해시 반환
fn build_root_tree(repo: &Repo, index: &Index) -> Result<String> {
    let mut root = TreeNode::default();
    for entry in &index.entries {
        let parts: Vec<String> = entry.path.split('/').map(|s| s.to_string()).collect();
        root.insert(&parts, &entry.hash, &entry.mode);
    }
    write_tree_node(repo, &root)
}

/// TreeNode 를 재귀적으로 tree 객체로 저장하고 해시 반환
fn write_tree_node(repo: &Repo, node: &TreeNode) -> Result<String> {
    let mut tree = Tree::new();

    // 파일 엔트리
    for (name, (hash, mode)) in &node.files {
        let entry = if mode == "100755" {
            TreeEntry::executable(name.clone(), hash.clone())
        } else {
            TreeEntry::file(name.clone(), hash.clone())
        };
        tree.add_entry(entry);
    }

    // 하위 디렉토리 엔트리 (먼저 자식 tree 저장 → 해시 참조)
    for (name, child) in &node.dirs {
        let child_hash = write_tree_node(repo, child)?;
        tree.add_entry(TreeEntry::directory(name.clone(), child_hash));
    }

    objects::write_tree(repo, &mut tree)
}
