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
mod repository_id;
mod repository_name;

pub use repository_id::RepositoryId;
pub use repository_name::RepositoryName;