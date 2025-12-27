// ============================================
// Repository API 라우트
// ============================================
// URL과 핸들러를 연결하는 라우팅 설정
//
// 예상 엔드포인트:
//   POST   /api/repositories          - 저장소 생성
//   GET    /api/repositories          - 저장소 목록
//   GET    /api/repositories/:id      - 저장소 상세
//   DELETE /api/repositories/:id      - 저장소 삭제
//   GET    /api/repositories/:id/tree - 코드 트리
//   GET    /api/repositories/:id/blob - 파일 내용
//
// Axum 라우터 예시:
//
// use axum::{Router, routing::{get, post, delete}};
// use super::handlers;
//
// pub fn routes() -> Router {
//     Router::new()
//         .route("/api/repositories", post(handlers::create_repository))
//         .route("/api/repositories", get(handlers::list_repositories))
//         .route("/api/repositories/:id", get(handlers::get_repository))
//         .route("/api/repositories/:id", delete(handlers::delete_repository))
//         .route("/api/repositories/:id/tree", get(handlers::get_tree))
//         .route("/api/repositories/:id/blob/:path", get(handlers::get_blob))
// }
