// =============================================================================
// 전역 자격증명 (credentials.rs)
// =============================================================================
//
// 서버 URL 별 인증 토큰을 전역 파일에 저장한다.
//   경로: $XDG_CONFIG_HOME/cts/credentials.json  (없으면 ~/.config/cts/...)
//
// 로그인은 저장소가 아니라 서버 단위이므로 .cts/config 가 아닌 전역에 둔다.
//
// 파일 위치: crates/cli/src/credentials.rs
// =============================================================================

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCred {
    pub token: String,
    pub username: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(default)]
    pub servers: BTreeMap<String, ServerCred>,
}

/// 서버 URL 정규화 (뒤쪽 슬래시 제거)
fn normalize(server: &str) -> String {
    server.trim_end_matches('/').to_string()
}

fn path() -> Result<PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .context("XDG_CONFIG_HOME / HOME 을 찾을 수 없습니다")?;
    Ok(base.join("cts").join("credentials.json"))
}

impl Credentials {
    pub fn load() -> Result<Self> {
        let p = path()?;
        if !p.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&p)
            .with_context(|| format!("자격증명 읽기 실패: {}", p.display()))?;
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }

    pub fn save(&self) -> Result<()> {
        let p = path()?;
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn get(&self, server: &str) -> Option<&ServerCred> {
        self.servers.get(&normalize(server))
    }

    pub fn set(&mut self, server: &str, cred: ServerCred) {
        self.servers.insert(normalize(server), cred);
    }
}

/// 특정 서버의 토큰 조회 (없으면 None)
pub fn token_for(server: &str) -> Result<Option<String>> {
    Ok(Credentials::load()?.get(server).map(|c| c.token.clone()))
}
