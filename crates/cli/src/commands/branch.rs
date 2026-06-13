// =============================================================================
// cts branch (commands/branch.rs)
// =============================================================================
//
//   cts branch          # 브랜치 목록 (현재 브랜치는 *)
//   cts branch <name>   # 현재 커밋에서 새 브랜치 생성
//
// 파일 위치: crates/cli/src/commands/branch.rs
// =============================================================================

use anyhow::{anyhow, bail, Result};

use crate::refs;
use crate::repo::Repo;

pub fn run(name: Option<String>) -> Result<()> {
    let repo = Repo::discover()?;
    match name {
        None => list(&repo),
        Some(n) => {
            let head = create_branch(&repo, &n)?;
            println!("브랜치 생성: {n} (at {})", &head[..head.len().min(10)]);
            Ok(())
        }
    }
}

fn list(repo: &Repo) -> Result<()> {
    let current = refs::current_branch(repo)?;
    let branches = refs::list_branches(repo)?;
    if branches.is_empty() {
        println!("(브랜치가 없습니다 — 첫 커밋을 만드세요)");
        return Ok(());
    }
    for b in branches {
        let mark = if b == current { "*" } else { " " };
        println!("{mark} {b}");
    }
    Ok(())
}

/// 현재 브랜치의 head 커밋에서 새 브랜치를 만든다. (head 해시 반환)
///
/// checkout -b 에서도 재사용한다.
pub fn create_branch(repo: &Repo, name: &str) -> Result<String> {
    validate_name(name)?;
    if refs::branch_exists(repo, name) {
        bail!("이미 존재하는 브랜치입니다: {name}");
    }
    let current = refs::current_branch(repo)?;
    let head = refs::read_branch(repo, &current)?
        .ok_or_else(|| anyhow!("커밋이 없어 브랜치를 만들 수 없습니다. 'cts commit' 을 먼저 실행하세요."))?;
    refs::update_branch(repo, name, &head)?;
    Ok(head)
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("브랜치 이름이 비어 있습니다");
    }
    let ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '.'));
    if !ok || name.starts_with('/') || name.ends_with('/') || name.contains("..") {
        bail!("브랜치 이름이 올바르지 않습니다: {name}");
    }
    Ok(())
}
