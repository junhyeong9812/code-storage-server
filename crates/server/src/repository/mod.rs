// =============================================================================
// Repository 도메인 (Bounded Context)
// =============================================================================
//
// 저장소, 브랜치, 커밋, 트리, Blob 관리
//
// 구조:
// - domain: 핵심 비즈니스 로직 (순수)
// - application: 유스케이스
// - infrastructure: 외부 시스템 연동 (DB, Storage)
// - api: REST API

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod api;
