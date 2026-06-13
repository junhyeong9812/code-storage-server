// =============================================================================
// cts log (commands/log.rs)
// =============================================================================
//
// 현재 브랜치의 커밋 히스토리를 최신순으로 출력한다.
// HEAD → 브랜치 head 커밋 → parent → ... 체인을 따라간다.
//
// 파일 위치: crates/cli/src/commands/log.rs
// =============================================================================

use anyhow::Result;

use crate::objects;
use crate::refs;
use crate::repo::Repo;

pub fn run() -> Result<()> {
    let repo = Repo::discover()?;
    let branch = refs::current_branch(&repo)?;

    let mut current = refs::read_branch(&repo, &branch)?;
    if current.is_none() {
        println!("아직 커밋이 없습니다 (브랜치 '{branch}').");
        return Ok(());
    }

    while let Some(hash) = current {
        let commit = objects::read_commit(&repo, &hash)?;
        println!("commit {hash}");
        println!(
            "Author: {} <{}>",
            commit.author_name, commit.author_email
        );
        println!("Date:   {}", commit.timestamp);
        println!();
        for line in commit.message.lines() {
            println!("    {line}");
        }
        println!();

        current = commit.parent_hash.clone();
    }

    Ok(())
}
