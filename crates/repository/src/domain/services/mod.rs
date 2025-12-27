// ============================================
// Repository 도메인 서비스
// ============================================
// 도메인 서비스: 특정 엔티티에 속하지 않는 비즈니스 로직
//
// 언제 도메인 서비스를 쓰나요?
//   - 여러 엔티티에 걸친 로직
//   - 엔티티 메서드로 표현하기 어색한 로직
//   - 외부 의존성 없이 순수한 도메인 로직
//
// 예시:
//   - RepositoryNameValidator: 저장소 이름 유효성 검사
//   - BranchMergeChecker: 브랜치 병합 가능 여부 확인
//
// 주의: 도메인 서비스는 상태를 가지지 않음 (stateless)
//
// 예시 (나중에 구현):
//
// pub struct RepositoryNameValidator;
//
// impl RepositoryNameValidator {
//     /// 저장소 이름 유효성 검사
//     pub fn validate(name: &str) -> Result<(), ValidationError> {
//         if name.is_empty() {
//             return Err(ValidationError::Empty);
//         }
//         if name.len() > 100 {
//             return Err(ValidationError::TooLong);
//         }
//         if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
//             return Err(ValidationError::InvalidCharacter);
//         }
//         Ok(())
//     }
// }

mod repository_path_service;