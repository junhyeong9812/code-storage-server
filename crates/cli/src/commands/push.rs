// =============================================================================
// cts push (commands/push.rs)
// =============================================================================
//
// 현재 브랜치 head 에서 도달 가능한 객체를 서버로 업로드하고 브랜치를 갱신한다.
//
// 파일 위치: crates/cli/src/commands/push.rs
// =============================================================================

use anyhow::{anyhow, Result};
use shared::protocol::PushRequest;

use crate::bundle;
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
    let head = refs::read_branch(&repo, &branch)?
        .ok_or_else(|| anyhow!("커밋이 없습니다. 'cts commit' 을 먼저 실행하세요."))?;

    let token = credentials::token_for(&remote.url)?
        .ok_or_else(|| anyhow!("먼저 'cts login {} <username>' 로 로그인하세요.", remote.url))?;

    let objects = bundle::collect_for_push(&repo, &head)?;
    let request = PushRequest {
        branch: branch.clone(),
        commit_hash: head.clone(),
        objects,
    };

    let resp = net::push(&remote, &request, &token)?;
    println!("푸시 완료 → {} ({branch})", remote.url);
    println!(
        "  신규: blob {}, tree {}, commit {}",
        resp.stored_blobs, resp.stored_trees, resp.stored_commits
    );
    Ok(())
}
