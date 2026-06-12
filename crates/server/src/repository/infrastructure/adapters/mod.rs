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

pub mod postgres_repository_repository;

pub use postgres_repository_repository::PgRepositoryRepository;
