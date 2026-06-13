// =============================================================================
// Repository 어댑터 (adapters/mod.rs)
// =============================================================================
//
// 인프라 레이어: 도메인 포트의 실제 구현체.
// - PgRepositoryRepository: PostgreSQL 기반 RepositoryRepository 구현
//
// (이후 Phase 에서 추가 예정)
// - PgCommitRepository
// - FileBlobStorage
//
// 파일 위치: crates/server/src/repository/infrastructure/adapters/mod.rs
// =============================================================================

pub mod file_blob_storage;
pub mod postgres_collaborator_repository;
pub mod postgres_object_repository;
pub mod postgres_repository_repository;

pub use file_blob_storage::FileBlobStorage;
pub use postgres_collaborator_repository::PgCollaboratorRepository;
pub use postgres_object_repository::PgObjectRepository;
pub use postgres_repository_repository::PgRepositoryRepository;
