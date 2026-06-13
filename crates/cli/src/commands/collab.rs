// =============================================================================
// cts collab (commands/collab.rs)
// =============================================================================
//
//   cts collab add <username> [role]   # 협업자 추가 (role 기본 write)
//   cts collab rm <username>           # 협업자 제거
//   cts collab ls                      # 협업자 목록
//
// 현재 저장소의 원격(config.remote) + 전역 토큰을 사용한다.
//
// 파일 위치: crates/cli/src/commands/collab.rs
// =============================================================================

use anyhow::{anyhow, Result};

use crate::config::Config;
use crate::credentials;
use crate::remote as net;
use crate::repo::Repo;

pub enum Action {
    Add { username: String, role: String },
    Rm { username: String },
    Ls,
}

pub fn run(action: Action) -> Result<()> {
    let repo = Repo::discover()?;
    let config = Config::load(&repo)?;
    let remote = config
        .remote
        .ok_or_else(|| anyhow!("원격이 없습니다. 'cts remote <url> <name>' 먼저 실행하세요."))?;
    let token = credentials::token_for(&remote.url)?;

    match action {
        Action::Add { username, role } => {
            let token = token
                .ok_or_else(|| anyhow!("로그인이 필요합니다: cts login {} <user>", remote.url))?;
            net::add_collaborator(&remote, &username, &role, &token)?;
            println!("협업자 추가: {username} ({role})");
        }
        Action::Rm { username } => {
            let token = token
                .ok_or_else(|| anyhow!("로그인이 필요합니다: cts login {} <user>", remote.url))?;
            net::remove_collaborator(&remote, &username, &token)?;
            println!("협업자 제거: {username}");
        }
        Action::Ls => {
            let list = net::list_collaborators(&remote, token.as_deref())?;
            if list.is_empty() {
                println!("(협업자 없음)");
            }
            for c in list {
                println!("  {:<6} {}", c.role, c.username);
            }
        }
    }
    Ok(())
}
