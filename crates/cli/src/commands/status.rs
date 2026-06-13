// =============================================================================
// cts status (commands/status.rs)
// =============================================================================
//
// 작업 트리 / 인덱스 / HEAD 커밋 트리의 3-way 비교 결과를 출력한다.
// 비교 로직은 worktree::compute 에 있다(checkout 과 공용).
//
// 파일 위치: crates/cli/src/commands/status.rs
// =============================================================================

use anyhow::Result;

use crate::repo::Repo;
use crate::worktree::{self, Change};

pub fn run() -> Result<()> {
    let repo = Repo::discover()?;
    let report = worktree::compute(&repo)?;

    println!("브랜치 {}", report.branch);

    if report.is_clean() {
        println!("커밋할 변경이 없으며 작업 트리가 깨끗합니다.");
        return Ok(());
    }

    if !report.staged.is_empty() {
        println!("\n커밋할 변경 사항:");
        print_changes(&report.staged);
    }
    if !report.not_staged.is_empty() {
        println!("\n스테이징되지 않은 변경:");
        print_changes(&report.not_staged);
    }
    if !report.untracked.is_empty() {
        println!("\n추적하지 않는 파일:");
        report.untracked.iter().for_each(|p| println!("  {p}"));
    }
    Ok(())
}

fn print_changes(changes: &[Change]) {
    for c in changes {
        println!("  {}   {}", c.kind.label(), c.path);
    }
}
