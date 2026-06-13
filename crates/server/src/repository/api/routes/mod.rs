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
        // Push: 객체 번들 업로드 + 브랜치 갱신
        .route("/repositories/:id/push", post(handlers::push_handler))
        // Pull: 객체 번들 다운로드
        .route("/repositories/:id/pull", get(handlers::pull_handler))
        // 브라우징(읽기) — Web UI
        .route("/repositories/:id/branches", get(handlers::branches_handler))
        .route("/repositories/:id/commits", get(handlers::commits_handler))
        .route(
            "/repositories/:id/tree/:commit_hash",
            get(handlers::tree_handler),
        )
        .route("/repositories/:id/blob/:hash", get(handlers::blob_handler))
        // 협업자 관리
        .route(
            "/repositories/:id/collaborators",
            post(handlers::add_collaborator_handler).get(handlers::list_collaborators_handler),
        )
        .route(
            "/repositories/:id/collaborators/:username",
            axum::routing::delete(handlers::remove_collaborator_handler),
        )
}
