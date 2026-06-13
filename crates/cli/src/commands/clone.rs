// =============================================================================
// cts clone (commands/clone.rs)
// =============================================================================
//
//   cts clone http://host:port/api/repositories/<id>
//
// 서버 저장소를 새 디렉토리로 복제한다(저장소 이름으로 디렉토리 생성 → pull).
//
// 파일 위치: crates/cli/src/commands/clone.rs
// =============================================================================

use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};

use crate::bundle;
use crate::checkout;
use crate::config::{Config, Remote};
use crate::credentials;
use crate::refs;
use crate::remote as net;
use crate::repo::Repo;

pub fn run(url: String) -> Result<()> {
    let (server, repo_id) = parse_url(&url)?;
    let token = credentials::token_for(&server)?;
    let info = net::get_repo(&server, &repo_id, token.as_deref())?;

    let dir = PathBuf::from(&info.name);
    if dir.exists() {
        bail!("대상 디렉토리가 이미 있습니다: {}", dir.display());
    }
    let repo = Repo::init(&dir)?;

    // 원격 설정 저장
    let mut config = Config::load(&repo)?;
    config.remote = Some(Remote {
        url: server.clone(),
        repo_id: repo_id.clone(),
        repo_name: Some(info.name.clone()),
    });
    config.save(&repo)?;

    // 기본 브랜치가 main 이 아니면 HEAD 갱신
    let branch = info.default_branch.clone();
    if branch != "main" {
        std::fs::write(repo.head_path(), format!("ref: refs/heads/{branch}\n"))?;
    }

    // pull
    let resp = net::pull(&server, &repo_id, &branch, token.as_deref())?;
    match resp.commit_hash {
        Some(head) => {
            bundle::apply_bundle(&repo, &resp.objects)?;
            refs::update_branch(&repo, &branch, &head)?;
            checkout::checkout(&repo, &head)?;
            println!(
                "클론 완료: {}/ ({branch} → {})",
                info.name,
                &head[..head.len().min(10)]
            );
        }
        None => {
            println!("클론 완료(빈 저장소): {}/", info.name);
        }
    }
    Ok(())
}

/// "http://host:port/api/repositories/<id>" → (server_base, repo_id)
fn parse_url(url: &str) -> Result<(String, String)> {
    const MARKER: &str = "/api/repositories/";
    let idx = url
        .find(MARKER)
        .ok_or_else(|| anyhow!("URL 형식: http://host:port/api/repositories/<id>"))?;
    let server = url[..idx].to_string();
    let repo_id = url[idx + MARKER.len()..].trim_end_matches('/').to_string();
    if server.is_empty() || repo_id.is_empty() {
        bail!("URL 형식 오류: {url}");
    }
    Ok((server, repo_id))
}
