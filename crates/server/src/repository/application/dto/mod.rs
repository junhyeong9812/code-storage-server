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
use crate::repository::domain::ports::{
    BranchHead, CollaboratorRecord, CommitRecord, TreeEntryRecord,
};

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

// -----------------------------------------------------------------------------
// 브라우징(읽기) DTO — Web UI 용
// -----------------------------------------------------------------------------

/// 브랜치 요약
#[derive(Debug, Serialize)]
pub struct BranchDto {
    pub name: String,
    pub head_commit: String,
}

impl From<BranchHead> for BranchDto {
    fn from(b: BranchHead) -> Self {
        Self {
            name: b.name,
            head_commit: b.commit_hash,
        }
    }
}

/// 커밋 요약
#[derive(Debug, Serialize)]
pub struct CommitSummary {
    pub hash: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub timestamp: String,
    pub parent_hash: Option<String>,
}

impl From<CommitRecord> for CommitSummary {
    fn from(c: CommitRecord) -> Self {
        Self {
            hash: c.hash,
            message: c.message,
            author_name: c.author_name,
            author_email: c.author_email,
            timestamp: c.timestamp,
            parent_hash: c.parent_hash,
        }
    }
}

/// 트리 엔트리
#[derive(Debug, Serialize)]
pub struct TreeEntryDto {
    pub name: String,
    pub object_type: String,
    pub hash: String,
    pub mode: String,
}

impl From<TreeEntryRecord> for TreeEntryDto {
    fn from(e: TreeEntryRecord) -> Self {
        Self {
            name: e.name,
            object_type: e.object_type,
            hash: e.child_hash,
            mode: e.mode,
        }
    }
}

/// 협업자 추가 요청
#[derive(Debug, Deserialize)]
pub struct AddCollaboratorRequest {
    pub username: String,
    /// "read" | "write" | "admin" (기본 write)
    #[serde(default)]
    pub role: Option<String>,
}

/// 협업자 응답
#[derive(Debug, Serialize)]
pub struct CollaboratorDto {
    pub user_id: Id,
    pub username: String,
    pub role: String,
}

impl From<CollaboratorRecord> for CollaboratorDto {
    fn from(c: CollaboratorRecord) -> Self {
        Self {
            user_id: c.user_id,
            username: c.username,
            role: c.role.as_str().to_string(),
        }
    }
}

/// blob 내용
#[derive(Debug, Serialize)]
pub struct BlobContentDto {
    pub hash: String,
    pub size: usize,
    /// UTF-8 텍스트면 true
    pub is_text: bool,
    /// 텍스트면 내용, 바이너리면 빈 문자열
    pub content: String,
}

impl BlobContentDto {
    pub fn from_bytes(hash: String, bytes: Vec<u8>) -> Self {
        let size = bytes.len();
        match String::from_utf8(bytes) {
            Ok(text) => Self {
                hash,
                size,
                is_text: true,
                content: text,
            },
            Err(_) => Self {
                hash,
                size,
                is_text: false,
                content: String::new(),
            },
        }
    }
}
