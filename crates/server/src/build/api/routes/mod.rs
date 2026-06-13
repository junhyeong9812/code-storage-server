// =============================================================================
// Build API 라우트 (routes/mod.rs)
// =============================================================================
//
// 저장소 하위의 빌드 엔드포인트. app() 에서 "/api" 아래에 nest 된다.
//   POST /api/repositories/:id/builds
//   GET  /api/repositories/:id/builds
//   GET  /api/repositories/:id/builds/:build_id
//   GET  /api/repositories/:id/builds/:build_id/log
//
// 파일 위치: crates/server/src/build/api/routes/mod.rs
// =============================================================================

use axum::{
    routing::{get, post},
    Router,
};

use super::handlers;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/repositories/:id/builds",
            post(handlers::trigger_handler).get(handlers::list_handler),
        )
        .route(
            "/repositories/:id/builds/:build_id",
            get(handlers::get_handler),
        )
        .route(
            "/repositories/:id/builds/:build_id/log",
            get(handlers::log_handler),
        )
}
