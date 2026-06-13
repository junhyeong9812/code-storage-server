// =============================================================================
// 저장소 설정 (config.rs)
// =============================================================================
//
// `.cts/config` (JSON) 에 저장되는 로컬 설정.
// - author_name / author_email: 커밋 작성자 (Phase 3)
// - remote: 원격 서버 URL (Phase 4 에서 사용)
//
// 파일 위치: crates/cli/src/config.rs
// =============================================================================

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::repo::Repo;

/// 원격 서버 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remote {
    /// 서버 베이스 URL (예: http://127.0.0.1:8080)
    pub url: String,
    /// 서버 측 저장소 ID (UUID 문자열)
    pub repo_id: String,
    /// 저장소 이름 (참고용)
    #[serde(default)]
    pub repo_name: Option<String>,
}

/// 로컬 저장소 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 커밋 작성자 이름
    pub author_name: String,
    /// 커밋 작성자 이메일
    pub author_email: String,
    /// 원격 서버 (Phase 4) — 'cts remote' 로 설정
    #[serde(default)]
    pub remote: Option<Remote>,
}

impl Config {
    /// init 시 사용할 기본 설정
    ///
    /// 작성자는 환경변수 USER/USERNAME 에서 추론, 없으면 "cts-user".
    pub fn default_for_init() -> Self {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "cts-user".to_string());
        Self {
            author_email: format!("{user}@cts.local"),
            author_name: user,
            remote: None,
        }
    }

    /// `.cts/config` 로드
    pub fn load(repo: &Repo) -> Result<Self> {
        let path = repo.config_path();
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("설정 파일을 읽을 수 없습니다: {}", path.display()))?;
        let config: Config = serde_json::from_str(&text).context("설정 파일 파싱 실패")?;
        Ok(config)
    }

    /// `.cts/config` 저장 (보기 좋은 JSON)
    pub fn save(&self, repo: &Repo) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(repo.config_path(), text)?;
        Ok(())
    }
}
