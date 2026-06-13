// =============================================================================
// CTS Server (lib.rs)
// =============================================================================
//
// 도메인별 Bounded Context 구조:
// - repository: 저장소, 브랜치, 커밋, 트리, Blob 관리
// - user: 사용자 인증/권한
// - build: CI/CD 빌드
//
// 공통(서버 루트) 모듈:
// - error: AppError → HTTP 응답 변환(ApiError)
// - state: 핸들러 공유 상태(AppState)
//
// app(state) 함수가 전체 라우터를 조립한다. main.rs 와 분리해 테스트하기 쉽게 한다.
// =============================================================================

pub mod auth;
pub mod error;
pub mod state;

pub mod build;
pub mod repository;
pub mod user;

use axum::{routing::get, Router};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// 전체 애플리케이션 라우터 조립
///
/// - `GET /health`            : 헬스체크
/// - `/api/...`               : 도메인별 REST API
pub fn app(state: AppState) -> Router {
    let api = repository::api::routes::routes()
        .merge(build::api::routes::routes())
        .merge(user::api::routes::routes());
    Router::new()
        .route("/health", get(health))
        .nest("/api", api)
        // Web UI(Vite 개발서버 :5173)에서의 교차 출처 요청 허용
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 헬스체크 핸들러
async fn health() -> &'static str {
    "ok"
}
