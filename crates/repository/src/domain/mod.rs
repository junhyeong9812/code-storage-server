// ============================================
// Repository 도메인 레이어
// ============================================
// 핵심 비즈니스 로직이 위치하는 곳
// 외부 의존성 없음 (순수한 Rust 코드)
//
// DDD 구성요소:
//   - entities: 엔티티 (고유 식별자를 가진 객체)
//   - value_objects: 값 객체 (불변, 식별자 없음)
//   - ports: 포트 (외부와의 인터페이스, 헥사고날)
//   - services: 도메인 서비스 (엔티티에 속하지 않는 로직)

pub mod entities;       // Repository, Commit, Branch 등
pub mod value_objects;  // RepositoryId, CommitHash, BranchName 등
pub mod ports;          // RepositoryPort, GitPort 등 (trait)
pub mod services;       // 도메인 서비스
