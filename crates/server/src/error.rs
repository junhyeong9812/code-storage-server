// =============================================================================
// API 에러 변환 (error.rs)
// =============================================================================
//
// shared::error::AppError 는 프레임워크에 비의존적인 순수 도메인 에러다.
// axum 의 IntoResponse 를 AppError 에 직접 구현할 수 없으므로(고아 규칙),
// 서버 크레이트에서 ApiError 뉴타입으로 감싸 HTTP 응답으로 변환한다.
//
// 핸들러는 `Result<T, ApiError>` 를 반환하고, `?` 로 AppError 를 전파하면
// From<AppError> 를 통해 자동으로 ApiError 로 변환된다.
//
// 파일 위치: crates/server/src/error.rs
// =============================================================================

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use shared::error::AppError;

/// AppError 를 HTTP 응답으로 변환하기 위한 래퍼
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        Self(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::AlreadyExists(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::HashMismatch { .. } => (StatusCode::BAD_REQUEST, self.0.to_string()),
            // Storage/Internal 은 내부 사정이므로 상세 메시지는 로그로, 응답은 일반화
            AppError::Storage(msg) | AppError::Internal(msg) => {
                tracing::error!(error = %msg, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
