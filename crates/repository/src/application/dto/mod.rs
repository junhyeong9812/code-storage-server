// ============================================
// Repository DTO (Data Transfer Object)
// ============================================
// DTO: 레이어 간 데이터 전송용 객체
//
// 왜 DTO를 쓰나요?
//   - 도메인 객체를 외부에 직접 노출하지 않음
//   - API 응답 형식과 도메인 모델 분리
//   - 필요한 필드만 선택적으로 노출
//
// DTO 종류:
//   - Input DTO: 요청 데이터 (CreateRepositoryInput)
//   - Output DTO: 응답 데이터 (RepositoryDto)
//
// 예시 (나중에 구현):
//
// use serde::{Deserialize, Serialize};
//
// /// 저장소 생성 요청
// #[derive(Deserialize)]
// pub struct CreateRepositoryInput {
//     pub name: String,
//     pub description: Option<String>,
// }
//
// /// 저장소 응답
// #[derive(Serialize)]
// pub struct RepositoryDto {
//     pub id: String,
//     pub name: String,
//     pub description: Option<String>,
//     pub created_at: String,
// }
//
// impl From<Repository> for RepositoryDto {
//     fn from(repo: Repository) -> Self {
//         Self {
//             id: repo.id.to_string(),
//             name: repo.name,
//             description: repo.description,
//             created_at: repo.created_at.to_rfc3339(),
//         }
//     }
// }
