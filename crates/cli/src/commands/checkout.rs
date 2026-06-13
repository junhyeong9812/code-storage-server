// =============================================================================
// cts checkout (commands/checkout.rs)
// =============================================================================
//
//   cts checkout <branch>       # 브랜치 전환 (작업 트리 갱신)
//   cts checkout -b <branch>    # 새 브랜치 생성 후 전환
//
// 안전장치: 커밋되지 않은 변경이 있으면 전환을 거부한다(데이터 손실 방지).
//
// 파일 위치: crates/cli/src/commands/checkout.rs
// =============================================================================

use anyhow::{anyhow, bail, Result};

use crate::checkout as restore;
use crate::commands::branch;
use crate::index::Index;
use crate::refs;
use crate::repo::Repo;
use crate::worktree;

pub fn run(branch_name: String, create: bool) -> Result<()> {
    let repo = Repo::discover()?;

    if create {
        branch::create_branch(&repo, &branch_name)?;
    }

    if !refs::branch_exists(&repo, &branch_name) {
        bail!("브랜치가 없습니다: {branch_name} (생성하려면 -b 옵션)");
    }

    let current = refs::current_branch(&repo)?;
    if current == branch_name {
        println!("이미 '{branch_name}' 브랜치입니다.");
        return Ok(());
    }

    // 커밋되지 않은 변경이 있으면 거부
    let status = worktree::compute(&repo)?;
    if status.has_uncommitted() {
        bail!("커밋되지 않은 변경이 있어 전환할 수 없습니다. 먼저 'cts commit' 하세요.");
    }

    let target_head = refs::read_branch(&repo, &branch_name)?
        .ok_or_else(|| anyhow!("대상 브랜치에 커밋이 없습니다: {branch_name}"))?;

    // 현재 추적 파일 제거 후 대상 스냅샷 복원
    let index = Index::load(&repo)?;
    remove_tracked_files(&repo, &index)?;
    restore::checkout(&repo, &target_head)?;
    refs::set_head(&repo, &branch_name)?;

    println!(
        "'{branch_name}' 브랜치로 전환했습니다 ({}).",
        &target_head[..target_head.len().min(10)]
    );
    Ok(())
}

/// 인덱스에 기록된(추적 중인) 파일들을 작업 트리에서 제거한다.
///
/// 새 브랜치 스냅샷 복원 전에 호출해 이전 브랜치의 파일이 남지 않게 한다.
/// (untracked 파일은 건드리지 않는다)
fn remove_tracked_files(repo: &Repo, index: &Index) -> Result<()> {
    for entry in &index.entries {
        let path = repo.root.join(&entry.path);
        if path.is_file() {
            std::fs::remove_file(&path).ok();
            // 비게 된 상위 디렉토리 정리 (루트는 제외)
            let mut dir = path.parent().map(|p| p.to_path_buf());
            while let Some(d) = dir {
                if d == repo.root || std::fs::remove_dir(&d).is_err() {
                    break;
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }
    Ok(())
}
