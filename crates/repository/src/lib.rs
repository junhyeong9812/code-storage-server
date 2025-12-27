// ============================================
// repository 크레이트 진입점 (lib.rs)
// ============================================
// Repository 도메인 (Bounded Context)
// Git 저장소의 생성, 관리, 코드 브라우징 담당
//
// 디렉토리 구조 (DDD + Hexagonal + Layered):
//   src/
//   ├── domain/          # 도메인 레이어 (핵심 비즈니스 로직)
//   ├── application/     # 응용 레이어 (유스케이스)
//   ├── infrastructure/  # 인프라 레이어 (외부 시스템 연동)
//   └── api/             # 표현 레이어 (REST API)

pub mod domain;          // 도메인 레이어
pub mod application;     // 응용 레이어
pub mod infrastructure;  // 인프라 레이어
pub mod api;             // 표현 레이어

// 의존성 방향:
//   api → application → domain ← infrastructure
//
// domain은 아무것도 의존하지 않음 (순수)
// infrastructure가 domain의 port(인터페이스)를 구현
