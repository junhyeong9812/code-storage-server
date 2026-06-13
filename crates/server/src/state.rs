// =============================================================================
// 애플리케이션 상태 (state.rs)
// =============================================================================
//
// axum 핸들러가 공유하는 의존성 묶음. 모든 도메인 포트를 Arc<dyn ...> 로 보관.
//
// 파일 위치: crates/server/src/state.rs
// =============================================================================

use std::sync::Arc;

use crate::build::domain::ports::{BuildRepository, BuildRunner};
use crate::repository::domain::ports::{
    BlobStorage, CollaboratorRepository, ObjectRepository, RepositoryRepository,
};
use crate::user::domain::ports::{PasswordHasher, TokenService, UserRepository};

/// 핸들러에 주입되는 공유 상태
#[derive(Clone)]
pub struct AppState {
    /// 저장소 메타데이터 영속화 포트
    pub repositories: Arc<dyn RepositoryRepository>,
    /// 협업자 영속화 포트
    pub collaborators: Arc<dyn CollaboratorRepository>,
    /// 객체 그래프(blob메타/tree/commit/branch) 영속화 포트
    pub objects: Arc<dyn ObjectRepository>,
    /// Blob 내용 저장소 포트
    pub blobs: Arc<dyn BlobStorage>,
    /// 빌드 기록 영속화 포트
    pub builds: Arc<dyn BuildRepository>,
    /// 빌드 실행기 포트
    pub build_runner: Arc<dyn BuildRunner>,
    /// 사용자 영속화 포트
    pub users: Arc<dyn UserRepository>,
    /// 비밀번호 해시 포트
    pub password_hasher: Arc<dyn PasswordHasher>,
    /// 토큰 발급/검증 포트
    pub tokens: Arc<dyn TokenService>,
}
