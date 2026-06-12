// =============================================================================
// Repository DTO (dto/mod.rs)
// =============================================================================
//
// DTO(Data Transfer Object): API 경계에서 주고받는 데이터 구조.
// - 요청(Request): 외부 → 서버 (역직렬화)
// - 응답(Response): 서버 → 외부 (직렬화)
//
// 도메인 엔티티(Repository)와 분리하는 이유:
// - 엔티티의 불변식/비공개 필드를 외부에 노출하지 않음
// - API 스키마와 도메인 모델을 독립적으로 진화시킬 수 있음
//
// 파일 위치: crates/server/src/repository/application/dto/mod.rs
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::types::{Id, Timestamp};

use crate::repository::domain::entities::Repository;

/// 저장소 생성 요청
#[derive(Debug, Deserialize)]
pub struct CreateRepositoryRequest {
    /// 저장소 이름 (검증은 도메인 RepositoryName 에서)
    pub name: String,
    /// 설명 (선택)
    #[serde(default)]
    pub description: Option<String>,
    /// 비공개 여부 (기본 false)
    #[serde(default)]
    pub is_private: bool,
}

/// 저장소 응답
#[derive(Debug, Serialize)]
pub struct RepositoryResponse {
    pub id: Id,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Id,
    pub default_branch: String,
    pub is_private: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl From<Repository> for RepositoryResponse {
    fn from(repo: Repository) -> Self {
        Self {
            id: repo.id().as_uuid(),
            name: repo.name().as_str().to_string(),
            description: repo.description().map(|s| s.to_string()),
            owner_id: repo.owner_id().as_uuid(),
            default_branch: repo.default_branch().to_string(),
            is_private: repo.is_private(),
            created_at: repo.created_at(),
            updated_at: repo.updated_at(),
        }
    }
}
