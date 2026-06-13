// =============================================================================
// 원격 통신 (remote.rs)
// =============================================================================
//
// 서버 REST API 호출 (ureq, 동기). 인증이 필요한 호출은 Bearer 토큰을 보낸다.
//
// 파일 위치: crates/cli/src/remote.rs
// =============================================================================

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use shared::protocol::{PullResponse, PushRequest, PushResponse};

use crate::config::Remote;

/// 서버 저장소 정보 (응답 일부만 사용)
#[derive(Debug, Deserialize)]
pub struct RepoInfo {
    pub id: String,
    pub name: String,
    #[serde(default = "default_branch")]
    pub default_branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

/// 인증 응답
#[derive(Debug, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: AuthUser,
}

#[derive(Debug, Deserialize)]
pub struct AuthUser {
    pub username: String,
}

fn base(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// ureq 에러를 읽기 좋은 anyhow 에러로 변환
fn map_err(e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            anyhow!("서버 오류 {code}: {body}")
        }
        ureq::Error::Transport(t) => anyhow!("연결 오류: {t}"),
    }
}

/// 요청에 Bearer 토큰 부착 (있으면)
fn auth(req: ureq::Request, token: Option<&str>) -> ureq::Request {
    match token {
        Some(t) => req.set("Authorization", &format!("Bearer {t}")),
        None => req,
    }
}

// -----------------------------------------------------------------------------
// 인증
// -----------------------------------------------------------------------------

pub fn register(server: &str, username: &str, email: &str, password: &str) -> Result<AuthResponse> {
    let url = format!("{}/api/auth/register", base(server));
    ureq::post(&url)
        .send_json(serde_json::json!({
            "username": username, "email": email, "password": password
        }))
        .map_err(map_err)?
        .into_json()
        .context("회원가입 응답 파싱 실패")
}

pub fn login(server: &str, username: &str, password: &str) -> Result<AuthResponse> {
    let url = format!("{}/api/auth/login", base(server));
    ureq::post(&url)
        .send_json(serde_json::json!({ "username": username, "password": password }))
        .map_err(map_err)?
        .into_json()
        .context("로그인 응답 파싱 실패")
}

/// 로그아웃 (서버에서 현재 토큰 철회)
pub fn logout(server: &str, token: &str) -> Result<()> {
    let url = format!("{}/api/auth/logout", base(server));
    auth(ureq::post(&url), Some(token)).call().map_err(map_err)?;
    Ok(())
}

// -----------------------------------------------------------------------------
// 저장소
// -----------------------------------------------------------------------------

/// 저장소 생성 (이미 있으면 목록에서 찾아 반환). 인증 필요.
pub fn create_or_get_repo(server: &str, name: &str, token: &str) -> Result<RepoInfo> {
    let url = format!("{}/api/repositories", base(server));
    match auth(ureq::post(&url), Some(token))
        .send_json(serde_json::json!({ "name": name }))
    {
        Ok(resp) => resp.into_json().context("저장소 생성 응답 파싱 실패"),
        Err(ureq::Error::Status(409, _)) => find_repo_by_name(server, name, Some(token))?
            .ok_or_else(|| anyhow!("이미 존재한다고 했으나 목록에서 찾지 못함: {name}")),
        Err(e) => Err(map_err(e)),
    }
}

pub fn get_repo(server: &str, repo_id: &str, token: Option<&str>) -> Result<RepoInfo> {
    let url = format!("{}/api/repositories/{}", base(server), repo_id);
    auth(ureq::get(&url), token)
        .call()
        .map_err(map_err)?
        .into_json()
        .context("저장소 조회 응답 파싱 실패")
}

fn find_repo_by_name(server: &str, name: &str, token: Option<&str>) -> Result<Option<RepoInfo>> {
    let url = format!("{}/api/repositories", base(server));
    let list: Vec<RepoInfo> = auth(ureq::get(&url), token)
        .call()
        .map_err(map_err)?
        .into_json()
        .context("저장소 목록 파싱 실패")?;
    Ok(list.into_iter().find(|r| r.name == name))
}

// -----------------------------------------------------------------------------
// push / pull
// -----------------------------------------------------------------------------

pub fn push(remote: &Remote, request: &PushRequest, token: &str) -> Result<PushResponse> {
    let url = format!(
        "{}/api/repositories/{}/push",
        base(&remote.url),
        remote.repo_id
    );
    auth(ureq::post(&url), Some(token))
        .send_json(request)
        .map_err(map_err)?
        .into_json()
        .context("push 응답 파싱 실패")
}

pub fn pull(
    server: &str,
    repo_id: &str,
    branch: &str,
    token: Option<&str>,
) -> Result<PullResponse> {
    let url = format!("{}/api/repositories/{}/pull", base(server), repo_id);
    auth(ureq::get(&url), token)
        .query("branch", branch)
        .call()
        .map_err(map_err)?
        .into_json()
        .context("pull 응답 파싱 실패")
}

// -----------------------------------------------------------------------------
// 협업자
// -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CollaboratorInfo {
    pub username: String,
    pub role: String,
}

pub fn add_collaborator(remote: &Remote, username: &str, role: &str, token: &str) -> Result<()> {
    let url = format!(
        "{}/api/repositories/{}/collaborators",
        base(&remote.url),
        remote.repo_id
    );
    auth(ureq::post(&url), Some(token))
        .send_json(serde_json::json!({ "username": username, "role": role }))
        .map_err(map_err)?;
    Ok(())
}

pub fn remove_collaborator(remote: &Remote, username: &str, token: &str) -> Result<()> {
    let url = format!(
        "{}/api/repositories/{}/collaborators/{}",
        base(&remote.url),
        remote.repo_id,
        username
    );
    auth(ureq::delete(&url), Some(token))
        .call()
        .map_err(map_err)?;
    Ok(())
}

pub fn list_collaborators(remote: &Remote, token: Option<&str>) -> Result<Vec<CollaboratorInfo>> {
    let url = format!(
        "{}/api/repositories/{}/collaborators",
        base(&remote.url),
        remote.repo_id
    );
    auth(ureq::get(&url), token)
        .call()
        .map_err(map_err)?
        .into_json()
        .context("협업자 목록 파싱 실패")
}
