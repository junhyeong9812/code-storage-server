// =============================================================================
// Repository 엔티티 (Aggregate Root)
// =============================================================================
//
// 저장소(Repository)는 이 Bounded Context의 애그리거트 루트다.
// 브랜치/커밋/트리/Blob은 모두 하나의 저장소에 속한다.
//
// 불변식(invariant):
// - name 은 항상 검증된 RepositoryName
// - owner_id 는 항상 존재
// - default_branch 는 비어 있지 않음 (기본 "main")
//
// 파일 위치: crates/server/src/repository/domain/entities/repository.rs
// =============================================================================

use shared::types::{now, Timestamp};

use crate::repository::domain::value_objects::{RepositoryId, RepositoryName, UserId};

/// 기본 브랜치 이름 (DB default 와 일치)
pub const DEFAULT_BRANCH: &str = "main";

/// 저장소 애그리거트 루트
#[derive(Debug, Clone)]
pub struct Repository {
    id: RepositoryId,
    name: RepositoryName,
    description: Option<String>,
    owner_id: UserId,
    default_branch: String,
    is_private: bool,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl Repository {
    /// 새 저장소 생성
    ///
    /// id 와 타임스탬프는 자동 생성되고 default_branch 는 "main" 으로 시작한다.
    pub fn new(
        name: RepositoryName,
        description: Option<String>,
        owner_id: UserId,
        is_private: bool,
    ) -> Self {
        let ts = now();
        Self {
            id: RepositoryId::generate(),
            name,
            description,
            owner_id,
            default_branch: DEFAULT_BRANCH.to_string(),
            is_private,
            created_at: ts,
            updated_at: ts,
        }
    }

    /// 영속 계층(DB)에서 읽은 값으로 엔티티 재구성
    ///
    /// 이미 저장된 데이터이므로 검증/생성 로직을 건너뛴다.
    #[allow(clippy::too_many_arguments)]
    pub fn from_persistence(
        id: RepositoryId,
        name: RepositoryName,
        description: Option<String>,
        owner_id: UserId,
        default_branch: String,
        is_private: bool,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Self {
        Self {
            id,
            name,
            description,
            owner_id,
            default_branch,
            is_private,
            created_at,
            updated_at,
        }
    }

    // -------------------------------------------------------------------------
    // 게터 (불변식 보호를 위해 필드는 비공개)
    // -------------------------------------------------------------------------
    pub fn id(&self) -> RepositoryId {
        self.id
    }
    pub fn name(&self) -> &RepositoryName {
        &self.name
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub fn owner_id(&self) -> UserId {
        self.owner_id
    }
    pub fn default_branch(&self) -> &str {
        &self.default_branch
    }
    pub fn is_private(&self) -> bool {
        self.is_private
    }
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_defaults() {
        let name = RepositoryName::parse("demo").unwrap();
        let owner = UserId::generate();
        let repo = Repository::new(name, Some("desc".into()), owner, false);

        assert_eq!(repo.name().as_str(), "demo");
        assert_eq!(repo.default_branch(), "main");
        assert_eq!(repo.owner_id(), owner);
        assert!(!repo.is_private());
        assert_eq!(repo.created_at(), repo.updated_at());
    }
}
