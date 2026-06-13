// =============================================================================
// cts login / cts register (commands/login.rs)
// =============================================================================
//
//   cts register <server_url> <username> <email>   # 가입 후 토큰 저장
//   cts login <server_url> <username>              # 로그인 후 토큰 저장
//
// 비밀번호는 숨김 입력. 토큰은 전역 자격증명(서버 URL별)에 저장한다.
//
// 파일 위치: crates/cli/src/commands/login.rs
// =============================================================================

use anyhow::{Context, Result};

use crate::credentials::{Credentials, ServerCred};
use crate::remote as net;

/// 비밀번호 입력 — 환경변수 CTS_PASSWORD 가 있으면 사용(비대화/CI), 없으면 숨김 프롬프트.
fn read_password() -> Result<String> {
    if let Ok(p) = std::env::var("CTS_PASSWORD") {
        return Ok(p);
    }
    rpassword::prompt_password("비밀번호: ").context("비밀번호 입력 실패")
}

pub fn register(server: String, username: String, email: String) -> Result<()> {
    let password = read_password()?;
    let resp = net::register(&server, &username, &email, &password)?;
    store(&server, &resp.token, &resp.user.username)?;
    println!("가입 완료: {} ({server})", resp.user.username);
    Ok(())
}

pub fn login(server: String, username: String) -> Result<()> {
    let password = read_password()?;
    let resp = net::login(&server, &username, &password)?;
    store(&server, &resp.token, &resp.user.username)?;
    println!("로그인 완료: {} ({server})", resp.user.username);
    Ok(())
}

fn store(server: &str, token: &str, username: &str) -> Result<()> {
    let mut creds = Credentials::load()?;
    creds.set(
        server,
        ServerCred {
            token: token.to_string(),
            username: username.to_string(),
        },
    );
    creds.save()
}
