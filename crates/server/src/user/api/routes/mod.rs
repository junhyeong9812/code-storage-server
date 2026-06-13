// =============================================================================
// User API 라우트 (routes/mod.rs)
// =============================================================================
//
//   POST /api/auth/register
//   POST /api/auth/login
//   GET  /api/users/me
//
// 파일 위치: crates/server/src/user/api/routes/mod.rs
// =============================================================================

use axum::{
    routing::{get, post},
    Router,
};

use super::handlers;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(handlers::register_handler))
        .route("/auth/login", post(handlers::login_handler))
        .route("/auth/logout", post(handlers::logout_handler))
        .route("/users/me", get(handlers::me_handler))
}
