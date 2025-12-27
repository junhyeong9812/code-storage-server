// ============================================
// Repository API 핸들러
// ============================================
// HTTP 요청을 처리하는 함수들
// 요청 파싱 → 유스케이스 호출 → 응답 반환
//
// Axum 핸들러 예시:
//
// use axum::{
//     extract::{Path, State, Json},
//     http::StatusCode,
//     response::IntoResponse,
// };
// use crate::application::{
//     use_cases::CreateRepositoryUseCase,
//     dto::{CreateRepositoryInput, RepositoryDto},
// };
//
// /// 저장소 생성 핸들러
// /// POST /api/repositories
// pub async fn create_repository(
//     State(use_case): State<CreateRepositoryUseCase>,  // DI로 주입된 유스케이스
//     Json(input): Json<CreateRepositoryInput>,         // 요청 본문 파싱
// ) -> Result<Json<RepositoryDto>, AppError> {
//     let result = use_case.execute(input).await?;
//     Ok(Json(result))
// }
//
// /// 저장소 목록 핸들러
// /// GET /api/repositories
// pub async fn list_repositories(
//     State(use_case): State<ListRepositoriesUseCase>,
// ) -> Result<Json<Vec<RepositoryDto>>, AppError> {
//     let result = use_case.execute().await?;
//     Ok(Json(result))
// }
//
// /// 저장소 상세 핸들러
// /// GET /api/repositories/:id
// pub async fn get_repository(
//     State(use_case): State<GetRepositoryUseCase>,
//     Path(id): Path<String>,  // URL 경로에서 id 추출
// ) -> Result<Json<RepositoryDto>, AppError> {
//     let result = use_case.execute(&id).await?;
//     Ok(Json(result))
// }
