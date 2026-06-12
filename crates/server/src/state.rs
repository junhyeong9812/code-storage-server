// =============================================================================
// 애플리케이션 상태 (state.rs)
// =============================================================================
//
// axum 핸들러가 공유하는 의존성 묶음.
// 도메인 포트를 Arc<dyn ...> 로 보관해 핸들러가 구체 구현(Postgres)을
// 모르게 한다(의존성 역전 유지).
//
// AppState 는 Clone 가능해야 한다(axum 요구). Arc 라서 clone 은 저렴하다.
//
// 파일 위치: crates/server/src/state.rs
// =============================================================================

use std::sync::Arc;

use crate::repository::domain::ports::RepositoryRepository;

/// 핸들러에 주입되는 공유 상태
#[derive(Clone)]
pub struct AppState {
    /// 저장소 영속화 포트
    pub repositories: Arc<dyn RepositoryRepository>,
}

impl AppState {
    pub fn new(repositories: Arc<dyn RepositoryRepository>) -> Self {
        Self { repositories }
    }
}
