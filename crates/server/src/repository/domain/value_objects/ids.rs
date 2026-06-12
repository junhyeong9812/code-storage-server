// =============================================================================
// ID 값 객체들 (ids.rs)
// =============================================================================
//
// 각 엔티티의 식별자를 "뉴타입(newtype)"으로 감싼다.
//
// 왜 Uuid를 그대로 안 쓰고 감싸나요?
// - 타입 안전성: RepositoryId 자리에 실수로 UserId를 넣으면 컴파일 에러
// - 의미 명확화: 함수 시그니처만 봐도 어떤 ID인지 알 수 있음
// - shared::types::Id(=Uuid) 위에 도메인 의미를 입힘
//
// 파일 위치: crates/server/src/repository/domain/value_objects/ids.rs
// =============================================================================

use serde::{Deserialize, Serialize};
use shared::types::{new_id, Id};

// -----------------------------------------------------------------------------
// define_id! 매크로
// -----------------------------------------------------------------------------
// 모든 ID 뉴타입이 동일한 보일러플레이트(생성/변환/Display)를 가지므로
// 선언적 매크로로 한 번만 정의하고 재사용한다.
macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Id);

        impl $name {
            /// 새 ID 생성 (UUID v4)
            pub fn generate() -> Self {
                Self(new_id())
            }

            /// 기존 UUID로부터 생성 (DB 로드 등)
            pub fn from_uuid(id: Id) -> Self {
                Self(id)
            }

            /// 내부 UUID 반환
            pub fn as_uuid(&self) -> Id {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Id> for $name {
            fn from(id: Id) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Id {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

define_id!(
    /// 저장소 식별자
    RepositoryId
);
define_id!(
    /// 사용자 식별자 (소유자)
    UserId
);
define_id!(
    /// 브랜치 식별자
    BranchId
);
define_id!(
    /// 커밋 식별자
    CommitId
);
define_id!(
    /// 트리 식별자
    TreeId
);
define_id!(
    /// Blob 식별자
    BlobId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_is_unique() {
        let a = RepositoryId::generate();
        let b = RepositoryId::generate();
        assert_ne!(a, b);
    }

    #[test]
    fn roundtrip_uuid() {
        let id = RepositoryId::generate();
        let uuid = id.as_uuid();
        assert_eq!(RepositoryId::from_uuid(uuid), id);
    }
}
