use std::fmt::Formatter;
use serde::{Deserialize, Serialize};
use shared::types::Id;

// 저장소 고유 식별자
// - 타입 안전성: UserId와 RepositoryId를 실수로 바꿔 쓰는 것 방지
// - 도메인 의미 부여: 그냥 UUID가 아니라 "저장소 ID"라는 의미
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryId(Id);

impl RepositoryId {
    pub fn new(id: Id) -> Self {
        Self(id)
    }

    pub fn generate() -> Self {
        Self(shared::types::new_id())
    }

    pub fn value(&self) -> &Id {
        &self.0
    }
}

// String으로 변환 (API 응답용)
impl std::fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
