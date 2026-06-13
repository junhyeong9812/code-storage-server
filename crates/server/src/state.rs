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

use crate::build::domain::ports::{BuildRepository, BuildRunner};
use crate::repository::domain::ports::{BlobStorage, ObjectRepository, RepositoryRepository};

/// 핸들러에 주입되는 공유 상태
#[derive(Clone)]
pub struct AppState {
    /// 저장소 메타데이터 영속화 포트
    pub repositories: Arc<dyn RepositoryRepository>,
    /// 객체 그래프(blob메타/tree/commit/branch) 영속화 포트
    pub objects: Arc<dyn ObjectRepository>,
    /// Blob 내용 저장소 포트
    pub blobs: Arc<dyn BlobStorage>,
    /// 빌드 기록 영속화 포트
    pub builds: Arc<dyn BuildRepository>,
    /// 빌드 실행기 포트
    pub build_runner: Arc<dyn BuildRunner>,
}

impl AppState {
    pub fn new(
        repositories: Arc<dyn RepositoryRepository>,
        objects: Arc<dyn ObjectRepository>,
        blobs: Arc<dyn BlobStorage>,
        builds: Arc<dyn BuildRepository>,
        build_runner: Arc<dyn BuildRunner>,
    ) -> Self {
        Self {
            repositories,
            objects,
            blobs,
            builds,
            build_runner,
        }
    }
}
