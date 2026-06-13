// =============================================================================
// cts pull (commands/pull.rs)
// =============================================================================
//
// 서버에서 현재 브랜치의 객체를 받아 로컬에 기록하고 작업 트리를 갱신한다.
//
// 파일 위치: crates/cli/src/commands/pull.rs
// =============================================================================

use anyhow::{anyhow, Result};

use crate::bundle;
use crate::checkout;
use crate::config::Config;
use crate::credentials;
use crate::refs;
use crate::remote as net;
use crate::repo::Repo;

pub fn run() -> Result<()> {
    let repo = Repo::discover()?;
    let config = Config::load(&repo)?;
    let remote = config
        .remote
        .ok_or_else(|| anyhow!("원격이 없습니다. 'cts remote <url> <name>' 을 먼저 실행하세요."))?;

    let branch = refs::current_branch(&repo)?;
    let token = credentials::token_for(&remote.url)?;
    let resp = net::pull(&remote.url, &remote.repo_id, &branch, token.as_deref())?;

    match resp.commit_hash {
        None => {
            println!("원격 브랜치 '{branch}' 에 커밋이 없습니다.");
        }
        Some(head) => {
            bundle::apply_bundle(&repo, &resp.objects)?;
            refs::update_branch(&repo, &branch, &head)?;
            checkout::checkout(&repo, &head)?;
            println!("풀 완료: {branch} → {}", &head[..head.len().min(10)]);
        }
    }
    Ok(())
}
