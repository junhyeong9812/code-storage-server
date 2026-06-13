// =============================================================================
// 원격 통신 (remote.rs)
// =============================================================================
//
// 서버 REST API 호출 (ureq, 동기).
// - 저장소 생성/조회/목록
// - push / pull
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

/// 저장소 생성 (이미 있으면 목록에서 찾아 ID 반환)
pub fn create_or_get_repo(server: &str, name: &str) -> Result<RepoInfo> {
    let url = format!("{}/api/repositories", base(server));
    match ureq::post(&url).send_json(serde_json::json!({ "name": name })) {
        Ok(resp) => resp.into_json().context("저장소 생성 응답 파싱 실패"),
        Err(ureq::Error::Status(409, _)) => find_repo_by_name(server, name)?
            .ok_or_else(|| anyhow!("이미 존재한다고 했으나 목록에서 찾지 못함: {name}")),
        Err(e) => Err(map_err(e)),
    }
}

/// ID 로 저장소 조회
pub fn get_repo(server: &str, repo_id: &str) -> Result<RepoInfo> {
    let url = format!("{}/api/repositories/{}", base(server), repo_id);
    ureq::get(&url)
        .call()
        .map_err(map_err)?
        .into_json()
        .context("저장소 조회 응답 파싱 실패")
}

/// 이름으로 저장소 찾기
fn find_repo_by_name(server: &str, name: &str) -> Result<Option<RepoInfo>> {
    let url = format!("{}/api/repositories", base(server));
    let list: Vec<RepoInfo> = ureq::get(&url)
        .call()
        .map_err(map_err)?
        .into_json()
        .context("저장소 목록 파싱 실패")?;
    Ok(list.into_iter().find(|r| r.name == name))
}

/// Push
pub fn push(remote: &Remote, request: &PushRequest) -> Result<PushResponse> {
    let url = format!(
        "{}/api/repositories/{}/push",
        base(&remote.url),
        remote.repo_id
    );
    ureq::post(&url)
        .send_json(request)
        .map_err(map_err)?
        .into_json()
        .context("push 응답 파싱 실패")
}

/// Pull
pub fn pull(server: &str, repo_id: &str, branch: &str) -> Result<PullResponse> {
    let url = format!("{}/api/repositories/{}/pull", base(server), repo_id);
    ureq::get(&url)
        .query("branch", branch)
        .call()
        .map_err(map_err)?
        .into_json()
        .context("pull 응답 파싱 실패")
}
