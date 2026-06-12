// =============================================================================
// Repository API 라우트 (routes/mod.rs)
// =============================================================================
//
// 저장소 관련 라우트를 하나의 Router 로 묶는다.
// 이 라우터는 app() 에서 "/api" 아래에 nest 된다.
//   → 최종 경로: /api/repositories, /api/repositories/:id
//
// 파일 위치: crates/server/src/repository/api/routes/mod.rs
// =============================================================================

use axum::{
    routing::{get, post},
    Router,
};

use super::handlers;
use crate::state::AppState;

/// 저장소 라우터 생성
pub fn routes() -> Router<AppState> {
    Router::new()
        // POST(생성) + GET(목록)
        .route(
            "/repositories",
            post(handlers::create_handler).get(handlers::list_handler),
        )
        // GET(조회) + DELETE(삭제)
        .route(
            "/repositories/:id",
            get(handlers::get_handler).delete(handlers::delete_handler),
        )
}
