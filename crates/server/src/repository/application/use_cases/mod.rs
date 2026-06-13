// =============================================================================
// Repository 유스케이스 (use_cases/mod.rs)
// =============================================================================
//
// 응용 레이어. 각 유스케이스는 "하나의 비즈니스 동작"을 표현한다.
// - 도메인 포트(RepositoryRepository)에만 의존
// - DB/HTTP 같은 구체 기술은 모른다
//
// Phase 2 범위: 저장소 CRUD
// - create_repository
// - get_repository
// - list_repositories
// - delete_repository
//
// (커밋/브랜치/빌드 유스케이스는 이후 Phase 에서 추가)
//
// 파일 위치: crates/server/src/repository/application/use_cases/mod.rs
// =============================================================================

pub mod browse;
pub mod create_repository;
pub mod delete_repository;
pub mod get_repository;
pub mod list_repositories;
pub mod pull;
pub mod push;

pub use browse::{browse_tree, list_branches, list_commits, read_blob};
pub use create_repository::create_repository;
pub use delete_repository::delete_repository;
pub use get_repository::get_repository;
pub use list_repositories::list_repositories;
pub use pull::pull;
pub use push::push;
