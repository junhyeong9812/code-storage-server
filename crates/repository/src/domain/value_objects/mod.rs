// ============================================
// Repository 값 객체 (Value Objects)
// ============================================
// 값 객체: 식별자 없이 값 자체로 동등성 판단
// 불변(immutable)이어야 함
//
// 이 모듈에 정의될 값 객체들:
//   - RepositoryId: 저장소 식별자
//   - RepositoryName: 저장소 이름 (유효성 검사 포함)
//   - CommitHash: 커밋 해시 (SHA-1)
//   - BranchName: 브랜치 이름
//
// 예시 (나중에 구현):
//
// #[derive(Clone, PartialEq, Eq, Hash)]
// pub struct RepositoryId(Uuid);
//
// impl RepositoryId {
//     pub fn new() -> Self {
//         Self(Uuid::new_v4())
//     }
// }
//
// #[derive(Clone, PartialEq, Eq)]
// pub struct RepositoryName(String);
//
// impl RepositoryName {
//     pub fn new(name: &str) -> Result<Self, ValidationError> {
//         // 유효성 검사: 빈 문자열, 특수문자 등
//         if name.is_empty() {
//             return Err(ValidationError::Empty);
//         }
//         Ok(Self(name.to_string()))
//     }
// }

mod repository_name;
mod repository_id;