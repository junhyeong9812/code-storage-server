// =============================================================================
// cts remote (commands/remote.rs)
// =============================================================================
//
//   cts remote <server_url> <repo_name>   # 원격 설정 (없으면 서버에 저장소 생성)
//   cts remote                            # 현재 원격 보기
//
// 파일 위치: crates/cli/src/commands/remote.rs
// =============================================================================

use anyhow::{bail, Result};

use crate::config::{Config, Remote};
use crate::remote as net;
use crate::repo::Repo;

pub fn run(url: Option<String>, name: Option<String>) -> Result<()> {
    let repo = Repo::discover()?;
    match (url, name) {
        (Some(url), Some(name)) => set(&repo, url, name),
        (None, None) => show(&repo),
        _ => bail!("사용법: cts remote <server_url> <repo_name>  또는  cts remote"),
    }
}

fn set(repo: &Repo, url: String, name: String) -> Result<()> {
    let info = net::create_or_get_repo(&url, &name)?;
    let mut config = Config::load(repo)?;
    config.remote = Some(Remote {
        url: url.clone(),
        repo_id: info.id.clone(),
        repo_name: Some(info.name.clone()),
    });
    config.save(repo)?;
    println!("원격 설정됨: {url} (repo '{}', id {})", info.name, info.id);
    Ok(())
}

fn show(repo: &Repo) -> Result<()> {
    let config = Config::load(repo)?;
    match config.remote {
        Some(r) => {
            let name = r
                .repo_name
                .map(|n| format!(", name '{n}'"))
                .unwrap_or_default();
            println!("원격: {} (repo_id {}{name})", r.url, r.repo_id);
        }
        None => println!("원격이 설정되지 않았습니다. 'cts remote <url> <name>' 으로 설정하세요."),
    }
    Ok(())
}
